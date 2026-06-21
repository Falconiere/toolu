#!/usr/bin/env bash
# Repo-wide Rust structural scanner — the deterministic backbone of the
# rust-refactor command (spec docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md).
#
# It resolves the Cargo workspace at --path, iterates its members, runs every
# per-file rule_* (and rule_layering_file) from the CANONICAL rule library over
# each .rs file, plus the cross-file rule_layering_crate once per crate, and
# reports the result as JSON (--json) or a human report.
#
# Define-once: this script SOURCES plugins/rust-quality/hooks/lib/rust-rules.sh
# (the very file register.sh concatenates into the per-edit gate), so the gate
# and the scan share one rule definition with zero drift. Rule logic is NOT
# reimplemented here — only the walk, the workspace resolution, and the report.
#
#   rust-scan.sh [--path <dir>] [--json] [--staged]
#
#     --path <dir>   workspace/crate root to scan (default: cwd)
#     --json         emit the spec's machine schema (default: human report)
#     --staged       limit the file set to `git diff --cached` .rs files
#
# A non-Rust / no-cargo / not-a-workspace target exits NON-ZERO with a clear
# stderr message — never an empty report an audit could misread as clean.

set -euo pipefail

# --------------------------------------------------------------------------
# Locate self + the canonical libraries.
# --------------------------------------------------------------------------
# Absolute dir of THIS script, so the worker re-exec and lib sourcing are
# path-independent (the script is invoked from anywhere, incl. via xargs).
RS_SELF="$(cd "${BASH_SOURCE[0]%/*}" && pwd)/${BASH_SOURCE[0]##*/}"
RS_SELF_DIR="${RS_SELF%/*}"
# plugins/rust-quality/scripts/ -> plugins/rust-quality/hooks/lib/
RS_RULES_DIR="$(cd "$RS_SELF_DIR/../hooks/lib" && pwd)"
# Core toolu lib (detect.sh, quality-config.sh) the rules depend on. In
# production the dispatcher exports TOOLU_LIB_DIR; fall back to the in-repo
# sibling so the scanner runs standalone.
if [ -z "${TOOLU_LIB_DIR:-}" ]; then
  TOOLU_LIB_DIR="$(cd "$RS_SELF_DIR/../../toolu/hooks/lib" && pwd)"
  export TOOLU_LIB_DIR
fi

# shellcheck source=../../toolu/hooks/lib/detect.sh
. "$TOOLU_LIB_DIR/detect.sh"
# shellcheck source=../../toolu/hooks/lib/quality-config.sh
. "$TOOLU_LIB_DIR/quality-config.sh"
# shellcheck source=../hooks/lib/rust-rules.sh
. "$RS_RULES_DIR/rust-rules.sh"

rs_die() { printf 'rust-scan: %s\n' "$1" >&2; exit "${2:-1}"; }

# --------------------------------------------------------------------------
# Worker mode: run every per-file rule over ONE .rs file and print TSV records.
# Invoked by the parallel fan-out (xargs) as `rust-scan.sh --rule-worker <file>`;
# kept as a re-exec (not an exported function) so each worker gets the libraries
# sourced cleanly under `set -e` without inheriting shell-function quirks.
# --------------------------------------------------------------------------
if [ "${1:-}" = "--rule-worker" ]; then
  rs_file="$2"
  [ -f "$rs_file" ] || exit 0
  # Each rule is pure and independent; a failure in one must not abort the file,
  # but a genuine rule error (a non-zero return — NOT "found violations", which is
  # still a 0 exit) MUST surface so the parallel fan-out never reports a partial
  # scan as clean. Run every rule, then exit non-zero if any errored; the parent
  # detects it via this worker's exit status (propagated by xargs).
  rs_worker_rc=0
  for rs_fn in rule_file_size rule_mod_rs_no_logic rule_generic_name \
               rule_test_location rule_module_doc rule_fn_size \
               rule_impl_size rule_layering_file; do
    "$rs_fn" "$rs_file" || rs_worker_rc=1
  done
  exit "$rs_worker_rc"
fi

# --------------------------------------------------------------------------
# Argument parsing.
# --------------------------------------------------------------------------
OPT_PATH=""
OPT_JSON=0
OPT_STAGED=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --path)   OPT_PATH="${2:-}"; shift 2 ;;
    --path=*) OPT_PATH="${1#--path=}"; shift ;;
    --json)   OPT_JSON=1; shift ;;
    --staged) OPT_STAGED=1; shift ;;
    -h|--help)
      sed -n '2,21p' "$RS_SELF"; exit 0 ;;
    *) rs_die "unknown argument: $1" 2 ;;
  esac
done

SCAN_PATH="${OPT_PATH:-$PWD}"
[ -d "$SCAN_PATH" ] || rs_die "not a directory: $SCAN_PATH" 2
# Canonicalize to the PHYSICAL path (-P resolves symlinks). On macOS `git
# rev-parse --show-toplevel` and `cargo metadata` both report physical paths
# (/private/var/…); a logical SCAN_PATH (/var/…) would then never match the
# staged-file set, silently emptying a --staged scan.
SCAN_PATH="$(cd "$SCAN_PATH" && pwd -P)"

command -v jq >/dev/null 2>&1 || rs_die "jq is required" 3

# --------------------------------------------------------------------------
# Workspace / crate resolution.
#
# Strategy (in order):
#   1. cargo present AND <path>/Cargo.toml parses -> use `cargo metadata`
#      members (authoritative; handles [workspace] + virtual manifests).
#   2. otherwise walk DOWN from <path> for Cargo.toml files and treat each
#      enclosing dir as a crate root. This covers a nested layout whose
#      Cargo.toml is below the scanned dir (no manifest AT <path>), and the
#      cargo-absent / unparseable-manifest fallback.
#
# CRATE_ROOTS is filled with one absolute crate-root dir per line.
# WORKSPACE_ROOT is the manifest anchor used for config-presence detection.
# A target with no Cargo.toml anywhere at or below it is non-Rust -> non-zero.
# --------------------------------------------------------------------------
CRATE_ROOTS=()
WORKSPACE_ROOT=""
# root -> crate name lookup, one `root<TAB>name` line per crate. crate_name()
# resolves a root's name from here, falling back to the dir basename.
CRATE_NAMES=""

crate_name() {
  local root="$1" n
  n="$(printf '%s\n' "$CRATE_NAMES" | awk -F '\t' -v r="$root" '$1 == r { print $2; exit }')"
  [ -n "$n" ] && { printf '%s' "$n"; return 0; }
  printf '%s' "${root##*/}"
}

resolve_via_cargo() {
  local manifest="$SCAN_PATH/Cargo.toml" meta
  [ -f "$manifest" ] || return 1
  command -v cargo >/dev/null 2>&1 || return 1
  meta="$(cargo metadata --format-version 1 --no-deps \
            --manifest-path "$manifest" 2>/dev/null)" || return 1
  [ -n "$meta" ] || return 1
  # --no-deps restricts packages[] to the workspace's own members.
  WORKSPACE_ROOT="$(jq -r '.workspace_root // empty' <<< "$meta")"
  [ -n "$WORKSPACE_ROOT" ] || return 1
  local pairs root name
  # Emit `manifest_dir<TAB>package_name` for each member.
  pairs="$(jq -r '.packages[]? | select(.manifest_path != null)
                  | "\(.manifest_path)\t\(.name)"' <<< "$meta")"
  [ -n "$pairs" ] || return 1
  while IFS=$'\t' read -r manifest_path name; do
    [ -n "$manifest_path" ] || continue
    root="$(dirname "$manifest_path")"
    CRATE_ROOTS+=("$root")
    CRATE_NAMES+="$root"$'\t'"$name"$'\n'
  done <<< "$pairs"
  return 0
}

resolve_via_walk() {
  # Find every Cargo.toml at or below SCAN_PATH; each enclosing dir is a crate
  # root. Prune target/ so a built workspace's compiled artifacts are skipped.
  # Names come from the manifest's `name = "…"`; basename if unparseable.
  local manifests m root name
  manifests="$(find "$SCAN_PATH" \
      -type d -name target -prune -o \
      -type f -name Cargo.toml -print 2>/dev/null | sort)"
  [ -n "$manifests" ] || return 1
  while IFS= read -r m; do
    [ -n "$m" ] || continue
    root="$(dirname "$m")"
    CRATE_ROOTS+=("$root")
    name="$(grep -m1 -E '^\s*name\s*=' "$m" 2>/dev/null \
              | sed -E 's/^[^"]*"([^"]*)".*$/\1/')"
    [ -n "$name" ] && CRATE_NAMES+="$root"$'\t'"$name"$'\n'
  done <<< "$manifests"
  # Prefer a manifest AT the scanned dir as the workspace anchor; else the
  # shallowest one found (first after sort by path depth).
  if [ -f "$SCAN_PATH/Cargo.toml" ]; then
    WORKSPACE_ROOT="$SCAN_PATH"
  else
    WORKSPACE_ROOT="${CRATE_ROOTS[0]}"
  fi
  return 0
}

if ! resolve_via_cargo; then
  CRATE_ROOTS=()
  WORKSPACE_ROOT=""
  CRATE_NAMES=""
  resolve_via_walk \
    || rs_die "not a Cargo workspace at $SCAN_PATH (no Cargo.toml at or below it)" 4
fi

[ "${#CRATE_ROOTS[@]}" -gt 0 ] \
  || rs_die "not a Cargo workspace at $SCAN_PATH (no crate members resolved)" 4

# --------------------------------------------------------------------------
# Scan-error sentinel. A scan that PARTIALLY failed (a crate that could not be
# read, a rule worker that errored, a layering pass that died) must never be
# reported as clean: each such failure appends a line here, and after the report
# is emitted the script exits non-zero (and the JSON carries an `errors` array).
# "Found violations" is NOT an error — that stays a 0 exit. Only genuine IO /
# permission / tool failures land here. The file is the cross-subshell channel:
# scan_crate runs inside $(...), so a flag variable would not propagate, but a
# real file write does. Cleaned up on exit.
# --------------------------------------------------------------------------
RS_ERR_FILE="$(mktemp)"
trap 'rm -f "$RS_ERR_FILE"' EXIT
rs_record_error() { printf '%s\n' "$1" >> "$RS_ERR_FILE"; }

# --------------------------------------------------------------------------
# Optional --staged filter: the set of staged .rs files (absolute paths).
# Built once; a member's file walk intersects against it.
# --------------------------------------------------------------------------
STAGED_SET=""
if [ "$OPT_STAGED" -eq 1 ]; then
  command -v git >/dev/null 2>&1 || rs_die "--staged requires git" 5
  git -C "$SCAN_PATH" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
    || rs_die "--staged: $SCAN_PATH is not inside a git work tree" 5
  local_top="$(git -C "$SCAN_PATH" rev-parse --show-toplevel)"
  # Resolve each staged path to absolute (relative to the git toplevel).
  STAGED_SET="$(git -C "$SCAN_PATH" diff --cached --name-only --diff-filter=ACMR \
                  -- '*.rs' 2>/dev/null \
                | while IFS= read -r rel; do
                    [ -n "$rel" ] && printf '%s/%s\n' "$local_top" "$rel"
                  done)"
fi

# is_staged FILE -> 0 if FILE is in the staged set (only consulted when --staged).
is_staged() {
  printf '%s\n' "$STAGED_SET" | grep -qxF "$1"
}

# --------------------------------------------------------------------------
# Per-crate file walk + rule fan-out.
#
# For each crate root, collect its .rs files (skip target/), optionally filter
# to staged, then run the rule worker over them with bounded parallelism. The
# combined TSV is emitted, prefixed with the crate root so the report builder
# can group rows by crate without re-deriving membership.
# --------------------------------------------------------------------------

# crate_rs_files ROOT -> newline list of .rs files under ROOT (target/ pruned).
crate_rs_files() {
  local root="$1"
  find "$root" -type d -name target -prune -o \
       -type f -name '*.rs' -print 2>/dev/null | sort
}

# scan_crate ROOT -> TSV rows, each prefixed `<root>\t` then the 6 rule fields.
scan_crate() {
  local root="$1" files filtered=""
  files="$(crate_rs_files "$root")"
  [ -n "$files" ] || return 0
  if [ "$OPT_STAGED" -eq 1 ]; then
    local f
    while IFS= read -r f; do
      [ -n "$f" ] || continue
      is_staged "$f" && filtered+="$f"$'\n'
    done <<< "$files"
    files="$filtered"
  fi
  [ -n "$files" ] || return 0
  # Bounded parallel fan-out: each file -> a rule-worker re-exec. xargs -P keeps
  # a 13-crate scan in single-digit seconds without spawning unbounded jobs.
  # PIPESTATUS[1] is xargs' own exit: it returns 123 when ANY rule worker exits
  # non-zero (a genuine rule error — workers exit 0 on "no violations"). pipefail
  # is on, but the worker rows are this function's stdout, so we read the status
  # explicitly rather than letting it abort, and record a sentinel on failure.
  # `{ <pipeline>; } || rc=$?` runs the pipeline for its rows (stdout passes
  # through) without letting `set -e` abort the function when a worker fails.
  # pipefail is on and the trailing awk always succeeds, so the captured status is
  # xargs' own: 123 if ANY worker exited non-zero — workers exit 0 on "no
  # violations", non-zero only on a genuine rule error.
  local fanout_rc=0
  { printf '%s\n' "$files" | awk 'NF' \
    | xargs -P 4 -I '{}' "$RS_SELF" --rule-worker '{}' \
    | awk -F '\t' -v r="$root" 'NF >= 6 { print r "\t" $0 }'; } || fanout_rc=$?
  if [ "$fanout_rc" -ne 0 ]; then
    rs_record_error "rule worker failed while scanning crate: $root"
  fi

  # Cross-file, repo-wide layering (AC-17, advisory). Unlike the per-file rules
  # this needs the WHOLE crate src/ to build its import graph, so it runs once
  # per crate here — not in the per-file worker. Skipped under --staged: a staged
  # subset cannot express cross-file layering, and a partial graph would be
  # misleading. Advisory rows are folded in with the same `<root>\t` prefix; they
  # ride into the JSON violations[] but never change the exit code.
  if [ "$OPT_STAGED" -eq 0 ]; then
    # Capture layering stderr instead of dropping it: its advisory rows are
    # informative, but a HARD failure (non-zero return) must surface — a partial
    # cross-file pass cannot be reported as a clean layering result.
    local layer_err
    layer_err="$(mktemp)"
    local layer_rc=0
    { rule_layering_crate "$root" 2>"$layer_err" \
      | awk -F '\t' -v r="$root" 'NF >= 6 { print r "\t" $0 }'; } || layer_rc=$?
    if [ "$layer_rc" -ne 0 ]; then
      rs_record_error "layering scan failed for crate $root: $(tr '\n' ' ' < "$layer_err")"
    fi
    rm -f "$layer_err"
  fi
  # Explicit success: in --staged mode the last evaluated statement is the
  # `[ "$OPT_STAGED" -eq 0 ]` test (false -> non-zero), which would otherwise make
  # the function look failed to the `if ! rows=...` guard in the driver.
  return 0
}

ALL_ROWS=""
for root in "${CRATE_ROOTS[@]}"; do
  # scan_crate records IO/worker/layering failures to the sentinel itself; the
  # `|| true` only stops `set -e` from aborting the whole run on one bad crate so
  # the remaining crates are still scanned. A scan_crate that exits non-zero
  # WITHOUT having recorded a sentinel (e.g. an unexpected internal failure) is
  # still surfaced here so the crate is never silently omitted as clean.
  if ! rows="$(scan_crate "$root")"; then
    grep -qF "crate $root" "$RS_ERR_FILE" 2>/dev/null \
      || rs_record_error "scan failed for crate: $root"
  fi
  [ -n "$rows" ] && ALL_ROWS+="$rows"$'\n'
done

# --------------------------------------------------------------------------
# Config presence — rustfmt.toml / clippy.toml / deny.toml at the workspace
# root, plus a [workspace.lints] table in the workspace Cargo.toml. Each is
# listed as present or missing (spec config.present / config.missing).
# --------------------------------------------------------------------------
CONFIG_PRESENT=()
CONFIG_MISSING=()
# config_check LABEL CANDIDATE...  — LABEL is present iff any CANDIDATE file
# exists at the workspace root; otherwise it is recorded missing.
config_check() {
  local label="$1"; shift
  local cand
  for cand in "$@"; do
    if [ -f "$cand" ]; then CONFIG_PRESENT+=("$label"); return 0; fi
  done
  CONFIG_MISSING+=("$label")
}
config_check rustfmt.toml "$WORKSPACE_ROOT/rustfmt.toml" "$WORKSPACE_ROOT/.rustfmt.toml"
config_check clippy.toml  "$WORKSPACE_ROOT/clippy.toml"  "$WORKSPACE_ROOT/.clippy.toml"
config_check deny.toml    "$WORKSPACE_ROOT/deny.toml"
# [workspace.lints] in the workspace Cargo.toml. ast-grep has no TOML grammar,
# so this is a literal section-header scan (a table header line, ignoring inline
# whitespace/comments) — a structural-shape match on text, not code.
if [ -f "$WORKSPACE_ROOT/Cargo.toml" ] \
   && grep -qE '^\s*\[workspace\.lints(\.[a-z]+)?\]' "$WORKSPACE_ROOT/Cargo.toml" 2>/dev/null; then
  CONFIG_PRESENT+=('Cargo.toml:[workspace.lints]')
else
  CONFIG_MISSING+=('Cargo.toml:[workspace.lints]')
fi

# --------------------------------------------------------------------------
# Build the report. The TSV (`<crate-root>\t<rule>\t<sev>\t<file>\t<line>\t<autofix>\t<msg>`)
# is the single source the JSON and human writers both consume.
# --------------------------------------------------------------------------
json_arr() {
  # Emit a JSON array of strings from newline-separated stdin (empty -> []).
  jq -R -s 'split("\n") | map(select(length > 0))'
}

emit_json() {
  # crates[] grouped by root, each with its violations[]; summary aggregates.
  # File paths are reported workspace-relative for portability; line/rule/etc.
  # pass through verbatim. jq does the grouping from the raw TSV in one pass.
  local crates_json
  crates_json="$(
    # Pass the crate roots (in order) so even a clean crate appears with [].
    printf '%s\n' "${CRATE_ROOTS[@]}" \
      | jq -R -s --arg ws "$WORKSPACE_ROOT" \
            --rawfile tsv <(printf '%s' "$ALL_ROWS") \
            --rawfile names <(printf '%s' "$CRATE_NAMES") '
          ($tsv | split("\n") | map(select(length > 0)) | map(split("\t"))) as $rows
          | ($names | split("\n") | map(select(length > 0)) | map(split("\t"))
             | map({ key: .[0], value: .[1] }) | from_entries) as $names
          | split("\n") | map(select(length > 0))
          | map(. as $root
              | { name: ($names[$root] // ($root | sub("^.*/"; ""))),
                  root: ($root | if startswith($ws + "/") then .[($ws|length)+1:]
                                 elif . == $ws then "." else . end),
                  violations: ( $rows
                    | map(select(.[0] == $root))
                    | map({ rule: .[1], severity: .[2],
                            file: ( .[3] | if startswith($ws + "/") then .[($ws|length)+1:] else . end ),
                            line: (.[4] | tonumber? // .[4]),
                            autofix: .[5], message: .[6] }) ) })
        '
  )"

  local present_json missing_json
  present_json="$(printf '%s\n' "${CONFIG_PRESENT[@]+"${CONFIG_PRESENT[@]}"}" | json_arr)"
  missing_json="$(printf '%s\n' "${CONFIG_MISSING[@]+"${CONFIG_MISSING[@]}"}" | json_arr)"

  jq -n \
    --arg repo "$WORKSPACE_ROOT" \
    --argjson crates "$crates_json" \
    --argjson present "$present_json" \
    --argjson missing "$missing_json" \
    --rawfile errs "$RS_ERR_FILE" '
    ( [ $crates[].violations[] ] ) as $all
    | ( $errs | split("\n") | map(select(length > 0)) ) as $errors
    | { repo: $repo,
        crates: $crates,
        config: { present: $present, missing: $missing },
        errors: $errors,
        summary: {
          total: ($all | length),
          byRule: ($all | group_by(.rule)
                        | map({ key: .[0].rule, value: length }) | from_entries),
          byAutofix: ($all | group_by(.autofix)
                          | map({ key: .[0].autofix, value: length }) | from_entries)
        } }
    '
}

emit_human() {
  local total
  total="$(printf '%s' "$ALL_ROWS" | awk -F '\t' 'NF >= 7 { c++ } END { print c+0 }')"
  printf 'rust-scan: %s\n' "$WORKSPACE_ROOT"
  printf '  crates: %d   violations: %d\n\n' "${#CRATE_ROOTS[@]}" "$total"
  local root
  for root in "${CRATE_ROOTS[@]}"; do
    local name rel
    name="$(crate_name "$root")"
    case "$root" in
      "$WORKSPACE_ROOT") rel="." ;;
      "$WORKSPACE_ROOT"/*) rel="${root#"$WORKSPACE_ROOT"/}" ;;
      *) rel="$root" ;;
    esac
    printf '  %s (%s)\n' "$name" "$rel"
    local crate_rows
    crate_rows="$(printf '%s' "$ALL_ROWS" \
      | awk -F '\t' -v r="$root" 'NF >= 7 && $1 == r { print }')"
    if [ -z "$crate_rows" ]; then
      printf '    (clean)\n'
    else
      printf '%s\n' "$crate_rows" | while IFS=$'\t' read -r _ rule sev file line _autofix msg; do
        local rfile="$file"
        case "$file" in "$WORKSPACE_ROOT"/*) rfile="${file#"$WORKSPACE_ROOT"/}" ;; esac
        printf '    [%s] %s:%s  %s — %s\n' "$sev" "$rfile" "$line" "$rule" "$msg"
      done
    fi
  done
  printf '\n  config present: %s\n' "${CONFIG_PRESENT[*]+"${CONFIG_PRESENT[*]}"}"
  printf '  config missing: %s\n' "${CONFIG_MISSING[*]+"${CONFIG_MISSING[*]}"}"
}

if [ "$OPT_JSON" -eq 1 ]; then
  emit_json
else
  emit_human
fi

# A partial failure (recorded to the sentinel by a crate scan, rule worker, or the
# layering pass) must NOT be reported as clean: the report above is still emitted
# (so the caller sees whatever WAS gathered, and --json carries the `errors`
# array), but the process exits non-zero with a clear message. Plain "violations
# found" never lands here — that stays a 0 exit.
if [ -s "$RS_ERR_FILE" ]; then
  printf 'rust-scan: scan did not complete cleanly:\n' >&2
  while IFS= read -r _rs_err; do
    [ -n "$_rs_err" ] && printf '  - %s\n' "$_rs_err" >&2
  done < "$RS_ERR_FILE"
  exit 6
fi
