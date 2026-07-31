#!/usr/bin/env bash
# Unified READ-ONLY view over the four toolu push gates (quality, plan, review,
# docs): "is this branch done, and why not" in one place, instead of four state
# files with no shared view. Never writes state and never runs a step's check —
# each gate function below MIRRORS the logic of its real enforcement point
# (quality-gate.sh, pre-tools/modules/{plan-ledger,push-review,docs-sync}.sh)
# without sourcing it, since those scripts are independently owned/evolving;
# drift between this view and a real gate is a bug to fix here, not a reason to
# couple to them.
#
# CLI:
#   bash plugins/toolu/hooks/lib/verdict.sh status   # human-readable table
#   bash plugins/toolu/hooks/lib/verdict.sh json     # machine-readable
#
# Exit codes: 0 ready, 1 blocked (>=1 gate fail/escalate), 2 error (not a git
# repo, or git/jq missing, or bad CLI usage).
#
# Push-review v2 schema (reviewed_files, version 2) is implemented HERE
# directly against the state file, independent of the push-review.sh gate
# itself still being v1 — see spec component 3 / component 7.

set -o pipefail

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
# shellcheck source=./detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=./diff-sha.sh
. "$_toolu_lib/diff-sha.sh"
# shellcheck source=./config.sh
. "$_toolu_lib/config.sh"
# shellcheck source=./docs-sync-config.sh
. "$_toolu_lib/docs-sync-config.sh"
# shellcheck source=./plan-ledger-parse.sh
. "$_toolu_lib/plan-ledger-parse.sh"

# Reviewer allow-list — keep in sync with pre-tools/modules/push-review.sh's
# accepted_reviewers (parity is not test-enforced across files; a mismatch
# would only surface as verdict disagreeing with the real gate).
ACCEPTED_REVIEWERS='["caveman:cavecrew-reviewer","code-review","toolu-review:review","code-review:xhigh","review","security-review"]'

# vd_gate STATE REASON [EXTRA_JSON]
# Print one gate object: {state, reason} merged with EXTRA_JSON (default "{}").
# EXTRA_JSON supplies gate-specific fields (plan's summary/ac_uncovered,
# review's reason_code/round) and must not itself carry state/reason.
vd_gate() {
  local state="$1" reason="$2" extra="$3"
  [ -n "$extra" ] || extra='{}'
  jq -cn --arg state "$state" --arg reason "$reason" --argjson extra "$extra" \
    '{state: $state, reason: $reason} + $extra'
}

# _vd_matches_any PATH GLOB_LIST -> 0 iff PATH fnmatches any newline-separated
# glob in GLOB_LIST. Local reimplementation of docs-sync.sh's own helper (that
# module is under concurrent edit; this is a ~10-line mirror, not a coupling).
_vd_matches_any() {
  local path="$1" glob
  while IFS= read -r glob; do
    [ -z "$glob" ] && continue
    # shellcheck disable=SC2254  # $glob is an intentional fnmatch pattern.
    case "$path" in
      $glob) return 0 ;;
    esac
  done <<< "$2"
  return 1
}

# vd_gate_quality REPO_ROOT
# Mirrors quality-gate.sh: linked worktree -> skip (gate never enforces there,
# checked BEFORE the file so a worktree always reads skip regardless of a
# stale/failing file left over from the main checkout); else read the
# multi-slot gate file's top-level status.
vd_gate_quality() {
  local repo_root="$1" gate_file git_dir common_dir

  git_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-dir 2>/dev/null || true)
  common_dir=$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
  git_dir="${git_dir%/}"
  common_dir="${common_dir%/}"
  if [ -n "$git_dir" ] && [ -n "$common_dir" ] && [ "$git_dir" != "$common_dir" ]; then
    vd_gate skip "gate disabled in linked worktree"
    return
  fi

  gate_file="$repo_root/.claude/tmp/quality-gate-status.json"
  if [ ! -f "$gate_file" ]; then
    vd_gate pass "no quality-gate failure recorded"
    return
  fi
  if [ "$(jq -r '.status // ""' "$gate_file" 2>/dev/null)" != "failing" ]; then
    vd_gate pass "quality gate passing"
    return
  fi
  vd_gate fail "$(jq -r '.reason // "Quality gate failing"' "$gate_file" 2>/dev/null || echo "Quality gate failing")"
}

# vd_gate_plan REPO_ROOT BRANCH BASE CUR
# Mirrors pre-tools/modules/plan-ledger.sh's fresh-green push check: absent
# ledger + code files in the diff -> advise; absent ledger + no code files, or
# an empty (no-op) ledger -> skip; unparseable/schema-mismatched ledger -> fail
# (fail closed, matching the real gate); else fail iff any step isn't
# fresh-green. Always reports summary{total,fresh_green} (recomputed against
# CUR, never trusting a possibly-stale on-disk summary) and a report-only
# ac_uncovered count (mirrors lib/plan-ledger.sh's pl_ac_coverage_lines).
vd_gate_plan() {
  local repo_root="$1" branch="$2" base="$3" cur="$4"
  local slug ledger_dir state_file zero_extra='{"summary":{"total":0,"fresh_green":0},"ac_uncovered":0}'
  slug=$(branch_slug "$branch")
  ledger_dir="${LEDGER_DIR:-$repo_root/.claude/tmp/plan-ledger}"
  state_file="$ledger_dir/${slug}.json"

  if [ ! -f "$state_file" ]; then
    local code_files
    code_files=$(git -C "$repo_root" diff --no-color "${base}...HEAD" --name-only 2>/dev/null \
      | grep -E '\.(ts|tsx|js|jsx|rs|sh|py|go)$' || true)
    if [ -n "$code_files" ]; then
      vd_gate advise "no plan ledger for this change; if non-trivial, run plan" "$zero_extra"
    else
      vd_gate skip "no plan ledger and no code files in diff" "$zero_extra"
    fi
    return
  fi

  local ledger
  if ! ledger=$(pl_read_ledger "$state_file" 2>/dev/null); then
    vd_gate fail "unparseable ledger at $state_file" "$zero_extra"
    return
  fi
  local version
  version=$(jq -r '.version // ""' <<< "$ledger" 2>/dev/null)
  if [ "$version" != "1" ]; then
    vd_gate fail "ledger schema mismatch at $state_file (version=\"$version\", expected 1)" "$zero_extra"
    return
  fi

  local total steps_count
  total=$(jq -r '.summary.total // 0' <<< "$ledger" 2>/dev/null || echo 0)
  steps_count=$(jq -r '.steps | length' <<< "$ledger" 2>/dev/null || echo 0)
  if [ "$total" = "0" ] || [ "$steps_count" = "0" ]; then
    vd_gate skip "empty plan ledger" "$zero_extra"
    return
  fi

  local summary_json
  summary_json=$(jq --arg cur "$cur" '
    def is_fresh: (.status == "green") and (.diff_sha == $cur);
    { total: (.steps | length), fresh_green: ([.steps[] | select(is_fresh)] | length) }
  ' <<< "$ledger")

  local blockers
  blockers=$(jq -r --arg cur "$cur" '
    .steps[]
    | (if (.status == "green" and .diff_sha == $cur) then "green"
       elif (.status == "green") then "stale"
       else (.status // "pending") end) as $eff
    | select($eff != "green")
    | (.id // "?") + ": " + $eff
  ' <<< "$ledger" 2>/dev/null || echo "")

  local ac_uncovered=0 plan_doc spec_field spec_abs acs_json
  plan_doc=$(jq -r '.plan_doc // ""' <<< "$ledger" 2>/dev/null || echo "")
  if [ -n "$plan_doc" ]; then
    local plan_abs="$plan_doc"
    case "$plan_doc" in
      /*) : ;;
      *) [ -f "$plan_abs" ] || plan_abs="$repo_root/$plan_doc" ;;
    esac
    if [ -f "$plan_abs" ]; then
      spec_field=$(pl_doc_field "$plan_abs" Spec)
      case "$(printf '%s' "$spec_field" | tr '[:upper:]' '[:lower:]')" in
        ""|none) : ;;
        *)
          spec_abs="$spec_field"
          case "$spec_field" in
            /*) : ;;
            *) [ -f "$spec_abs" ] || spec_abs="$repo_root/$spec_field" ;;
          esac
          if [ -f "$spec_abs" ]; then
            acs_json=$(pl_parse_acs "$spec_abs" | jq -Rsc 'split("\n") | map(select(length > 0))')
            if [ -n "$acs_json" ] && [ "$acs_json" != "[]" ]; then
              ac_uncovered=$(jq -n --argjson l "$ledger" --arg cur "$cur" --argjson acs "$acs_json" '
                [ $acs[] as $ac
                  | ([$l.steps[] | select((.ac_refs // []) | index($ac))]) as $cov
                  | ($cov | any(.status == "green" and .diff_sha == $cur)) as $fresh
                  | select($fresh | not)
                ] | length
              ' 2>/dev/null) || ac_uncovered=0
            fi
          fi
          ;;
      esac
    fi
  fi

  local extra
  extra=$(jq -cn --argjson summary "$summary_json" --argjson ac "$ac_uncovered" '{summary: $summary, ac_uncovered: $ac}')

  if [ -z "$blockers" ]; then
    vd_gate pass "all plan-ledger steps fresh-green" "$extra"
  else
    vd_gate fail "steps not fresh-green: $(printf '%s' "$blockers" | tr '\n' ',' | sed 's/,$//')" "$extra"
  fi
}

# vd_gate_review REPO_ROOT BRANCH BASE CUR
# Implements the push-review v2 schema directly (version normalizes to "2",
# reviewed_files must equal the sorted current diff) against the same state
# file pre-tools/modules/push-review.sh reads — independent of that gate's own
# v1-vs-v2 migration (spec component 3 + 7). reason_code is a closed set:
# empty-diff | stale-diff | schema-v1 | file-coverage | no-state | round-cap |
# findings | reviewer | pass.
vd_gate_review() {
  local repo_root="$1" branch="$2" base="$3" cur="$4"
  local empty_blob_sha="e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"

  if [ "$branch" = "$base" ]; then
    vd_gate skip "current branch is the base branch" '{"reason_code":null,"round":null}'
    return
  fi
  if [ -z "$cur" ]; then
    vd_gate skip "could not compute diff against $base" '{"reason_code":null,"round":null}'
    return
  fi
  if [ "$cur" = "$empty_blob_sha" ]; then
    vd_gate fail "diff against $base is empty; verify intent before pushing" \
      '{"reason_code":"empty-diff","round":null}'
    return
  fi

  local slug state_dir state_file
  slug=$(branch_slug "$branch")
  state_dir="${STATE_DIR:-$repo_root/.claude/tmp/push-review}"
  state_file="$state_dir/${slug}.json"

  if [ ! -f "$state_file" ]; then
    vd_gate fail "no push-review state file; run a reviewer and write the state" \
      '{"reason_code":"no-state","round":null}'
    return
  fi

  local state_json
  state_json=$(cat "$state_file" 2>/dev/null) || state_json=""
  if [ -z "$state_json" ] || ! jq -e . <<< "$state_json" >/dev/null 2>&1; then
    # Present-but-garbage is a schema problem, not an absence — "no-state" is
    # reserved for the file-absent case above, matching push-review.sh (an
    # unparseable state there falls through to its version-mismatch "schema"
    # deny, never its file-absent "no-state" one).
    vd_gate fail "push-review state file is unparseable at $state_file" \
      '{"reason_code":"schema","round":null}'
    return
  fi

  local round_json
  round_json=$(jq -c '(.review_round // 1) | if type == "number" then . else null end' <<< "$state_json" 2>/dev/null)
  [ -n "$round_json" ] || round_json="null"

  local version_norm state_sha state_findings reviewed_type
  version_norm=$(jq -r '(.version // "") | tostring' <<< "$state_json" 2>/dev/null)
  state_sha=$(jq -r '.diff_sha // ""' <<< "$state_json" 2>/dev/null)
  state_findings=$(jq -r '(.findings_count // "") | tostring' <<< "$state_json" 2>/dev/null)
  reviewed_type=$(jq -r '(.reviewed_files // null) | type' <<< "$state_json" 2>/dev/null)

  # schema-v1 is reserved for a state whose version normalizes to exactly "1"
  # (the one-time-upgrade case push-review.sh gives its own deny message for);
  # anything else malformed — version outside {1,2}, or a v2 missing a
  # required field — is the generic "schema" code, mirroring push-review.sh's
  # own split between its version==1 check and its version!=2-or-corrupt check.
  if [ "$version_norm" = "1" ]; then
    vd_gate fail "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"schema-v1", round:$round}')"
    return
  fi
  if [ "$version_norm" != "2" ] || [ -z "$state_sha" ] || [ -z "$state_findings" ] || [ "$reviewed_type" != "array" ]; then
    vd_gate fail "push-review state file is corrupted or missing required v2 fields at $state_file" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"schema", round:$round}')"
    return
  fi

  if ! jq -e --argjson acc "$ACCEPTED_REVIEWERS" \
       '(.reviewers // []) as $r | any($acc[]; . as $x | $r | index($x) != null)' \
       <<< "$state_json" >/dev/null 2>&1; then
    vd_gate fail "state file lists no accepted reviewer" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"reviewer", round:$round}')"
    return
  fi

  local round_num
  round_num=$(jq -r '(.review_round // 1) | if type == "number" then (floor|tostring) else "" end' <<< "$state_json" 2>/dev/null)
  if [ -n "$round_num" ] && [ "$round_num" -gt 5 ]; then
    vd_gate escalate "review loop hit $round_num rounds (max 5) on an unchanged diff" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"round-cap", round:$round}')"
    return
  fi

  if [ "$state_sha" != "$cur" ]; then
    vd_gate fail "diff changed since review (state stale)" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"stale-diff", round:$round}')"
    return
  fi

  local changed_sorted reviewed_sorted
  changed_sorted=$(git -C "$repo_root" diff --no-color "${base}...HEAD" --name-only 2>/dev/null | sort -u)
  reviewed_sorted=$(jq -r '.reviewed_files[]' <<< "$state_json" 2>/dev/null | sort -u)
  if [ "$changed_sorted" != "$reviewed_sorted" ]; then
    vd_gate fail "reviewed_files does not match the current diff's changed paths" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"file-coverage", round:$round}')"
    return
  fi

  if [ "$state_findings" != "0" ]; then
    vd_gate fail "code review has open findings ($state_findings)" \
      "$(jq -cn --argjson round "$round_json" '{reason_code:"findings", round:$round}')"
    return
  fi

  vd_gate pass "review satisfied" "$(jq -cn --argjson round "$round_json" '{reason_code:"pass", round:$round}')"
}

# vd_gate_docs REPO_ROOT BRANCH
# Mirrors pre-tools/modules/docs-sync.sh's has_code/has_doc classification
# (via docs-sync-config.sh's glob readers) and attestation check, reading the
# attestation dir via docs-sync's OWN path resolution — NOT REPO_ROOT — so a
# worktree + CLAUDE_PROJECT_DIR setup is found the same way the real writer
# resolves it. "fail" is only reachable under docsSync.mode=block.
vd_gate_docs() {
  local repo_root="$1" branch="$2" base mode

  base="${DOCS_SYNC_BASE:-$(detect_base_branch "$repo_root")}"
  mode=$(toolu_string "docsSync.mode" "advise" "advise" "block" "off")
  if [ "$mode" = "off" ]; then
    vd_gate skip "docsSync.mode is off"
    return
  fi

  if [ -z "$branch" ] || [ "$branch" = "HEAD" ]; then
    vd_gate skip "detached HEAD"
    return
  fi
  if ! git -C "$repo_root" rev-parse --verify --quiet "$base" >/dev/null 2>&1; then
    vd_gate skip "base branch '$base' not found locally"
    return
  fi
  if [ "$branch" = "$base" ]; then
    vd_gate skip "current branch is the base branch"
    return
  fi

  local changed
  changed=$(git -C "$repo_root" diff --no-color "${base}...HEAD" --name-only 2>/dev/null || echo "")
  if [ -z "$changed" ]; then
    vd_gate skip "no diff against $base"
    return
  fi

  local surfaces surface_excludes code_surfaces
  surfaces=$(docs_sync_surfaces)
  surface_excludes=$(docs_sync_surface_excludes)
  code_surfaces=$(docs_sync_code_surfaces)

  local has_doc=0 has_code=0 path
  while IFS= read -r path; do
    [ -z "$path" ] && continue
    if _vd_matches_any "$path" "$surfaces" && ! _vd_matches_any "$path" "$surface_excludes"; then
      has_doc=1
    fi
    if _vd_matches_any "$path" "$code_surfaces"; then has_code=1; fi
  done <<< "$changed"

  if [ "$has_code" != "1" ] || [ "$has_doc" = "1" ]; then
    vd_gate pass "doc surface in sync"
    return
  fi

  # Code changed, no doc surface touched. Attestation dir mirrors docs-sync.sh
  # EXACTLY (not repo_root): a worktree running with CLAUDE_PROJECT_DIR set
  # must find the same file the writer wrote.
  local diff_sha state_dir state_file
  diff_sha=$(toolu_diff_sha "$repo_root" "$base") || diff_sha=""
  state_dir="${DOCS_SYNC_STATE_DIR:-${CLAUDE_PROJECT_DIR:-$(pwd)}/.claude/tmp/docs-sync}"
  state_file="$state_dir/$(branch_slug "$branch").json"
  if [ -f "$state_file" ] && [ -n "$diff_sha" ]; then
    local attested_sha
    attested_sha=$(jq -r '.diff_sha // ""' "$state_file" 2>/dev/null || echo "")
    if [ "$attested_sha" = "$diff_sha" ]; then
      vd_gate pass "doc change attested as not needed"
      return
    fi
  fi

  if [ "$mode" = "block" ]; then
    vd_gate fail "code changed without a doc update (docsSync.mode=block)"
  else
    vd_gate advise "code changed without a doc update"
  fi
}

# vd_render_status JSON
# Print JSON's data as a compact, aligned human table (same data as `json`).
vd_render_status() {
  local json="$1" branch base sha overall gate state reason
  branch=$(jq -r '.branch' <<< "$json")
  base=$(jq -r '.base_branch' <<< "$json")
  sha=$(jq -r '.diff_sha' <<< "$json")
  overall=$(jq -r '.overall' <<< "$json")

  printf 'verdict: %s vs %s (diff %s)\n' "$branch" "$base" "${sha:0:12}"
  printf '%-8s %-9s %s\n' "GATE" "STATE" "REASON"
  for gate in quality plan review docs; do
    state=$(jq -r --arg g "$gate" '.gates[$g].state' <<< "$json")
    reason=$(jq -r --arg g "$gate" '.gates[$g].reason' <<< "$json")
    printf '%-8s %-9s %s\n' "$gate" "$state" "$reason"
  done
  printf 'overall: %s\n' "$overall"
}

# main MODE (status|json) -> print the report, return 0 ready / 1 blocked / 2 error.
main() {
  local mode="${1:-}"
  case "$mode" in
    status|json) : ;;
    *) echo "verdict: usage: status | json" >&2; return 2 ;;
  esac

  command -v git >/dev/null 2>&1 || { echo "verdict: git is required" >&2; return 2; }
  command -v jq  >/dev/null 2>&1 || { echo "verdict: jq is required" >&2; return 2; }

  local repo_root
  repo_root=$(git rev-parse --show-toplevel 2>/dev/null) || repo_root=""
  [ -n "$repo_root" ] || { echo "verdict: not a git repository" >&2; return 2; }

  local branch base cur
  branch=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  base="${PUSH_REVIEW_BASE:-$(detect_base_branch "$repo_root")}"
  cur=$(toolu_diff_sha "$repo_root" "$base") || cur=""

  local g_quality g_plan g_review g_docs
  g_quality=$(vd_gate_quality "$repo_root")
  g_plan=$(vd_gate_plan "$repo_root" "$branch" "$base" "$cur")
  g_review=$(vd_gate_review "$repo_root" "$branch" "$base" "$cur")
  g_docs=$(vd_gate_docs "$repo_root" "$branch")

  local overall
  if ! overall=$(jq -rn --argjson q "$g_quality" --argjson p "$g_plan" --argjson r "$g_review" --argjson d "$g_docs" '
    if ([$q, $p, $r, $d] | any(.state == "fail" or .state == "escalate"))
    then "blocked" else "ready" end
  ' 2>/dev/null) || [ -z "$overall" ]; then
    echo "verdict: failed to assemble gate results (one of the four gate functions produced empty/invalid JSON)" >&2
    return 2
  fi

  local out
  out=$(jq -n \
    --arg branch "$branch" --arg base "$base" --arg sha "$cur" \
    --arg overall "$overall" --arg now "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --argjson quality "$g_quality" --argjson plan "$g_plan" \
    --argjson review "$g_review" --argjson docs "$g_docs" '
    { version: 1, branch: $branch, base_branch: $base, diff_sha: $sha,
      overall: $overall,
      gates: { quality: $quality, plan: $plan, review: $review, docs: $docs },
      generated_at: $now }
  ')

  if [ "$mode" = "json" ]; then
    printf '%s\n' "$out"
  else
    vd_render_status "$out"
  fi

  [ "$overall" = "ready" ] && return 0 || return 1
}

# Guarded main: run only when executed directly, not when sourced.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  main "$@"
  exit $?
fi
