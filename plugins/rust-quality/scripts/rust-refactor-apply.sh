#!/usr/bin/env bash
# Deterministic apply pipeline for the rust-refactor command
# (spec docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md, Apply phase + AC-11).
#
# This is the MECHANICAL half of Apply — no agent judgement, no semantic file
# moves. It runs the four cheap+deterministic steps in spec order:
#
#   1. scaffold/merge config (+ [workspace.lints] propagation) via rust-scaffold.sh
#   2. cargo fmt --all
#   3. cargo clippy --fix (the deny lints define what --fix repairs)
#   4. a final rust-scan.sh --json re-audit, residuals grouped by autofix class
#
# Each mutating step is run through the AC-11 rollback helper `_apply_step`:
# it commits the step on the refactor branch, runs `cargo check`, and on failure
# `git reset --hard HEAD~1`s the step out and records it as a `manual` residual —
# so the tree is never left half-applied. The semantic restructure (split / rename
# / move) is the agent's job in the SKILL, not here.
#
#   rust-refactor-apply.sh --path <repo>
#
#     --path <repo>   the target Cargo workspace root (required); must already be
#                     on the refactor branch with a clean tree (see preflight).

set -euo pipefail

# --------------------------------------------------------------------------
# Locate self + sibling scripts.
# --------------------------------------------------------------------------
AP_SELF="$(cd "${BASH_SOURCE[0]%/*}" && pwd)/${BASH_SOURCE[0]##*/}"
AP_SELF_DIR="${AP_SELF%/*}"
AP_SCAFFOLD="$AP_SELF_DIR/rust-scaffold.sh"
AP_SCAN="$AP_SELF_DIR/rust-scan.sh"

ap_die() { printf 'rust-refactor-apply: %s\n' "$1" >&2; exit "${2:-1}"; }
ap_info() { printf 'rust-refactor-apply: %s\n' "$1"; }

# Residual steps that failed `cargo check` and were rolled back — reported as
# `manual` at the end (AC-11). One label per line.
AP_MANUAL_RESIDUALS=""

# --------------------------------------------------------------------------
# AC-11 rollback helper.
#
#   _apply_step <label> <cmd...>
#
# Runs <cmd...> in $REPO; if it succeeds and left changes, commits them on the
# refactor branch, then runs `cargo check`. On a check failure the commit is
# reverted (`git reset --hard HEAD~1`) and <label> is recorded as a `manual`
# residual. A step that makes no change is a clean no-op (nothing to commit).
#
# Kept a callable function (not inlined) so the e2e suite can drive it directly.
# Returns 0 on success/no-op/clean-rollback (the pipeline never aborts on a
# single recoverable step); returns non-zero only if the command itself could
# not run (e.g. missing tool), which the caller treats as fatal.
# --------------------------------------------------------------------------
_apply_step() {
  local label="$1"; shift
  ap_info "step: $label"

  # Run the step. A non-zero exit from the command is itself a failure mode we
  # roll back from (there is nothing committed yet, so just record + return).
  if ! ( cd "$REPO" && "$@" ); then
    ap_info "step '$label' command failed — recording as manual residual"
    AP_MANUAL_RESIDUALS+="$label"$'\n'
    return 0
  fi

  # Nothing changed -> clean no-op, nothing to commit or verify.
  if [ -z "$(git -C "$REPO" status --porcelain)" ]; then
    ap_info "step '$label' produced no changes"
    return 0
  fi

  # Commit the step so a failed `cargo check` can be reset out atomically.
  # INVARIANT: `git add -A` is safe to use here ONLY because the refactor preflight
  # (in the /rust-refactor SKILL) asserts a CLEAN tree before this pipeline runs —
  # so at apply-time the sole untracked/modified files are this step's own
  # mechanical output (scaffold config, fmt, clippy --fix). It never sweeps
  # pre-existing user work, because there is none.
  git -C "$REPO" add -A
  git -C "$REPO" commit --no-verify -q -m "rust-refactor: $label" \
    || ap_die "failed to commit step '$label'" 7

  # The compile oracle. A step that breaks the build is reverted, not kept.
  if ( cd "$REPO" && cargo check --all-targets >/dev/null 2>&1 ); then
    ap_info "step '$label' committed (cargo check ok)"
  else
    ap_info "step '$label' broke cargo check — rolling back to manual residual"
    git -C "$REPO" reset --hard HEAD~1 >/dev/null 2>&1 \
      || ap_die "rollback of step '$label' failed" 7
    AP_MANUAL_RESIDUALS+="$label"$'\n'
  fi
  return 0
}

# --------------------------------------------------------------------------
# Argument parsing.
# --------------------------------------------------------------------------
OPT_PATH=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --path)   OPT_PATH="${2:-}"; shift 2 ;;
    --path=*) OPT_PATH="${1#--path=}"; shift ;;
    -h|--help) sed -n '2,21p' "$AP_SELF"; exit 0 ;;
    *) ap_die "unknown argument: $1" 2 ;;
  esac
done

[ -n "$OPT_PATH" ] || ap_die "--path <repo> is required" 2
[ -d "$OPT_PATH" ] || ap_die "not a directory: $OPT_PATH" 2
REPO="$(cd "$OPT_PATH" && pwd -P)"

command -v git >/dev/null 2>&1 || ap_die "git is required" 3
command -v cargo >/dev/null 2>&1 || ap_die "cargo is required" 3
[ -x "$AP_SCAFFOLD" ] || ap_die "scaffolder not found: $AP_SCAFFOLD" 3
[ -x "$AP_SCAN" ] || ap_die "scanner not found: $AP_SCAN" 3

git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || ap_die "not inside a git work tree: $REPO" 5

# --------------------------------------------------------------------------
# The mechanical pipeline, each step gated by the rollback helper.
#
# Order is load-bearing (spec Apply): config scaffold establishes the deny lints
# clippy --fix repairs, so it MUST precede the clippy step; fmt runs between so
# the scaffolded config is itself well-formatted before clippy reads it.
# --------------------------------------------------------------------------
ap_info "applying mechanical refactor to $REPO"

# (1) Merge config + propagate [workspace.lints]. Deterministic, no judgement.
_apply_step "scaffold-config" "$AP_SCAFFOLD" --path "$REPO"

# (2) Format the whole workspace.
_apply_step "cargo-fmt" cargo fmt --all

# (3) Apply clippy's machine-applicable fixes. The dirty guard is WAIVED
#     (--allow-dirty --allow-staged): mid-refactor the tree carries the
#     scaffold + fmt commits, which clippy would otherwise refuse to touch.
_apply_step "clippy-fix" \
  cargo clippy --fix --allow-dirty --allow-staged --all-targets

# --------------------------------------------------------------------------
# (4) Final re-audit. Print remaining violations grouped by autofix class so the
# agent's restructure phase knows exactly what mechanical fixes did NOT resolve.
# --------------------------------------------------------------------------
ap_info "re-auditing $REPO"
# The final re-audit is load-bearing: its byAutofix breakdown is the report. A
# swallowed scan failure (the old `|| true`) would blank SCAN_JSON and print a
# bogus "clean" report. Capture stderr and the real exit code; a scan FAILURE
# (non-zero) aborts loudly. (rust-scan exits non-zero only on a genuine failure,
# not on "violations found", so a dirty-but-scannable repo still returns 0.)
SCAN_ERR="$(mktemp)"
if SCAN_JSON="$( "$AP_SCAN" --path "$REPO" --json 2>"$SCAN_ERR" )"; then
  rm -f "$SCAN_ERR"
else
  ap_info "re-audit scan failed:"
  cat "$SCAN_ERR" >&2
  rm -f "$SCAN_ERR"
  ap_die "final re-audit scan failed for $REPO — report is unreliable" 6
fi

if [ -n "$SCAN_JSON" ] && command -v jq >/dev/null 2>&1; then
  printf 'rust-refactor-apply: remaining violations by autofix class:\n'
  printf '%s' "$SCAN_JSON" | jq -r '
    ( .summary.byAutofix // {} ) as $by
    | if ($by | length) == 0
      then "  (none — all mechanical classes resolved)"
      else ( $by | to_entries
                 | sort_by(.key)
                 | map("  \(.key): \(.value)") | .[] )
      end
  '
else
  ap_info "re-audit produced no JSON (jq unavailable)"
fi

# --------------------------------------------------------------------------
# Report rolled-back steps as manual residuals (AC-11).
# --------------------------------------------------------------------------
if [ -n "$AP_MANUAL_RESIDUALS" ]; then
  printf 'rust-refactor-apply: manual residuals (rolled back, apply by hand):\n'
  printf '%s' "$AP_MANUAL_RESIDUALS" | awk 'NF { print "  - " $0 }'
else
  ap_info "no manual residuals — all mechanical steps applied cleanly"
fi

ap_info "done"
