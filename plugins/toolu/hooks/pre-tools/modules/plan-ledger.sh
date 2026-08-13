#!/usr/bin/env bash
# Pre-tool gate: block `git push` until every plan-ledger step is fresh-green.
# READ-ONLY — this gate never runs a step's `check`; it only reads the ledger
# the checker (plan-ledger.sh) wrote. A step is "fresh-green" iff its recorded
# status is green AND its recorded diff_sha equals the current branch diff_sha,
# so a green that predates a later code change is treated as stale, not done.
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload (also piped to stdin; this module reads the env var)
#
# Ledger file: <project-root>/<host-state>/tmp/plan-ledger/<branch-slug>.json
# Override the parent dir via $LEDGER_DIR for testing.
#
# Decisions:
#   - tool != Bash, or not a `git push`            -> allow (exit 0, no output).
#   - ledger absent, push diff touches a code file -> allow + stderr advisory.
#   - ledger absent, no code files                 -> allow silently.
#   - ledger unparseable / version != 1            -> deny (fail closed).
#   - summary.total == 0 / steps empty             -> allow (no-op).
#   - every step fresh-green                        -> allow.
#   - any red/pending/stale step                    -> deny, listing each.
#   - planLedger.blockOnUncoveredAcs=true + an UNCOVERED spec AC -> deny,
#     naming the AC id(s) (default false: advisory ac_coverage count only).

# pipefail so a `git diff | git hash-object` failure surfaces; without it an
# empty diff stream succeeds and yields the well-known empty-blob SHA.
set -o pipefail

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/plan-ledger-parse.sh
. "$_toolu_lib/plan-ledger-parse.sh"

[[ "$tool_name" != "Bash" ]] && exit 0

command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')

# Push detection (strip_heredocs + boundary-anchored regex) shared via detect.sh.
is_git_push "$command" || exit 0

# Sourced here (past the Edit/Write/Grep and non-push cheap exits above) so
# every other tool call skips the extra jq-merge/telemetry-lib load these four
# pull in — only an actual `git push` pays for them.
# shellcheck source=../../lib/diff-sha.sh
. "$_toolu_lib/diff-sha.sh"
# shellcheck source=../../lib/plan-ledger.sh
. "$_toolu_lib/plan-ledger.sh"
# shellcheck source=../../lib/telemetry.sh
. "$_toolu_lib/telemetry.sh"
# shellcheck source=../../lib/config.sh
. "$_toolu_lib/config.sh"

# Base branch: env override > detect_base_branch (must agree with the checker).
base_branch="${PUSH_REVIEW_BASE:-$(detect_base_branch)}"

current_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
slug=$(branch_slug "$current_branch")

# Ledger lives next to push-review's state, keyed by the same branch slug.
ledger_dir="${LEDGER_DIR:-$(toolu_project_state_dir plan-ledger "$(detect_project_root)")}"
state_file="$ledger_dir/${slug}.json"

# Ledger absent: never deny on absence. Nudge only when the push diff touches a
# code file (escape hatch for mechanical work — see spec Non-Goal 3).
if [[ ! -f "$state_file" ]]; then
  code_files=$(git diff --no-color "${base_branch}...HEAD" --name-only 2>/dev/null \
    | grep -E '\.(ts|tsx|js|jsx|rs|sh|py|go)$' || true)
  if [[ -n "$code_files" ]]; then
    echo "plan-ledger: no plan ledger for this change; if non-trivial, run plan" >&2
  fi
  exit 0
fi

# Read + validate the ledger. Unparseable -> fail closed (deny).
ledger=$(pl_read_ledger "$state_file") || {
  jq -n --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("plan-ledger: unparseable ledger at " + $file + "; delete and re-run `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc>`")
    }
  }'
  exit 0
}

# Schema gate: version must be 1 (fail closed on mismatch).
version=$(jq -r '.version // ""' <<<"$ledger" 2>/dev/null || echo "")
if [[ "$version" != "1" ]]; then
  jq -n --arg file "$state_file" --arg v "$version" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("plan-ledger: ledger schema mismatch at " + $file + " (version=\"" + $v + "\", expected 1); delete and re-run `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc>`")
    }
  }'
  exit 0
fi

# No-op ledger: total 0 or no steps -> allow (identical to absent, spec crit12).
total=$(jq -r '.summary.total // 0' <<<"$ledger" 2>/dev/null || echo "0")
step_count=$(jq -r '.steps | length' <<<"$ledger" 2>/dev/null || echo "0")
if [[ "$total" == "0" || "$step_count" == "0" ]]; then
  exit 0
fi

# Current branch diff_sha — the content hash the checker stamps when a step runs.
current_diff_sha=$(toolu_diff_sha . "$base_branch")
if [[ -z "$current_diff_sha" ]]; then
  # git diff failed (disk full, etc): allow so the underlying push surfaces the
  # real error rather than denying on an indeterminate read.
  echo "plan-ledger: git diff ${base_branch}...HEAD failed; allowing push to surface real error" >&2
  exit 0
fi

# ac_coverage telemetry (report-only counts; never affects the decision below).
# Mirrors lib/plan-ledger.sh's pl_cmd_status AC-coverage resolution: read the
# ledger's plan_doc, resolve its **Spec:** field, and reuse pl_ac_coverage_lines
# (sourced from lib/plan-ledger.sh above) to get the per-AC report, then count
# covered vs UNCOVERED lines in it. Spec-less plans print no report -> no event.
ac_plan_doc=$(jq -r '.plan_doc // ""' <<<"$ledger" 2>/dev/null || echo "")
if [[ -n "$ac_plan_doc" ]]; then
  ac_root=$(detect_project_root)
  [[ -f "$ac_plan_doc" ]] || { [[ -n "$ac_root" && -f "$ac_root/$ac_plan_doc" ]] && ac_plan_doc="$ac_root/$ac_plan_doc"; }
  if [[ -f "$ac_plan_doc" ]]; then
    ac_spec_field=$(pl_doc_field "$ac_plan_doc" Spec)
    ac_spec_path="$ac_spec_field"
    case "$(printf '%s' "$ac_spec_field" | tr '[:upper:]' '[:lower:]')" in
      ""|none) : ;;
      *) [[ -f "$ac_spec_path" ]] || { [[ -n "$ac_root" && -f "$ac_root/$ac_spec_field" ]] && ac_spec_path="$ac_root/$ac_spec_field"; } ;;
    esac
    ac_report=$(pl_ac_coverage_lines "$ledger" "$current_diff_sha" "$ac_spec_path" 2>/dev/null)
    if [[ -n "$ac_report" ]]; then
      ac_total=$(grep -c '^  AC-' <<<"$ac_report" || true)
      ac_uncovered=$(grep -Ec '^  AC-[^:]+: UNCOVERED' <<<"$ac_report" || true)
      ac_covered=$(( ac_total - ac_uncovered ))
      telemetry_append "$ac_root" "ac_coverage" \
        "$(jq -cn --argjson c "$ac_covered" --argjson u "$ac_uncovered" '{covered: $c, uncovered: $u}')"

      # Promotion (spec component 8): planLedger.blockOnUncoveredAcs (default
      # false) turns UNCOVERED spec ACs from an advisory count into a push
      # deny. The event above already recorded the counts either way.
      if [[ "$ac_uncovered" -gt 0 ]] && toolu_flag_true planLedger blockOnUncoveredAcs; then
        ac_uncovered_lines=$(grep -E '^  AC-[^:]+: UNCOVERED' <<<"$ac_report" | sed -E 's/^  //')
        ac_reason=$(printf 'plan-ledger: push blocked — uncovered spec AC id(s):\n%s\n\ncover with a fresh-green step referencing it in ac_refs, or set planLedger.blockOnUncoveredAcs=false' "$ac_uncovered_lines")
        jq -n --arg reason "$ac_reason" '{
          "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": $reason
          }
        }'
        exit 0
      fi
    fi
  fi
fi

# Build the list of non-fresh-green steps. A step is fresh-green iff
# status==green AND diff_sha==current; a green with a stale diff_sha is `stale`.
# `effective` is the status the agent sees: green -> stale when sha drifts.
blockers=$(jq -r --arg cur "$current_diff_sha" '
  .steps[]
  | (if (.status == "green" and .diff_sha == $cur) then "green"
     elif (.status == "green") then "stale"
     else (.status // "pending") end) as $eff
  | select($eff != "green")
  | (.id // "?") + ": " + $eff + " — " + (.title // "")
' <<<"$ledger" 2>/dev/null || echo "")

# All fresh-green -> allow.
if [[ -z "$blockers" ]]; then
  exit 0
fi

# Otherwise deny, listing each non-fresh-green step + remediation.
plan_doc=$(jq -r '.plan_doc // "<plan_doc>"' <<<"$ledger" 2>/dev/null || echo "<plan_doc>")
reason=$(printf 'plan-ledger: push blocked — steps not fresh-green:\n%s\n\nrun: bash plugins/toolu/hooks/lib/plan-ledger.sh run %s' "$blockers" "$plan_doc")
jq -n --arg reason "$reason" '{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": $reason
  }
}'
exit 0
