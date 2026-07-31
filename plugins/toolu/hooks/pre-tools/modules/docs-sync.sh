#!/usr/bin/env bash
# Pre-tool check: on `git push`, flag when the branch diff changes code but
# touches no documentation surface — the docs-sync backstop for the toolu
# workflow's "Docs in sync" convention. Silenced by a diff-sha-keyed
# attestation the agent writes when no doc change is warranted.
#
# Project-agnostic. Behavior is controlled by `docsSync.mode`
# (advise|block|off, via toolu_string — unrecognized values warn and fall
# back to advise):
#   - advise (default): emits hookSpecificOutput.additionalContext (merged by
#     dispatch.sh) and NEVER a permissionDecision — it informs, it does not
#     gate (push-review.sh is the gate).
#   - block: denies the push (permissionDecision: "deny") when code changed
#     with no doc surface and no matching attestation. A valid attestation
#     still allows the push in this mode.
#   - off: the check is fully disabled — no telemetry, no output, exit 0.
#     Not just "suppress the message": an off branch must be indistinguishable
#     from docs-sync not existing at all.
#
# Inputs (exported by pre-tools/mod.sh): $tool_name, $input (JSON, also stdin).
# Attestation: .claude/tmp/docs-sync/<branch-slug>.json (override $DOCS_SYNC_STATE_DIR).
# Base branch: $DOCS_SYNC_BASE override, else detect_base_branch.

# pipefail so a `git diff | git hash-object` failure surfaces rather than
# yielding the well-known empty-blob SHA from a broken pipe.
set -o pipefail

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/config.sh
. "$_toolu_lib/config.sh"
# shellcheck source=../../lib/docs-sync-config.sh
. "$_toolu_lib/docs-sync-config.sh"
# shellcheck source=../../lib/diff-sha.sh
. "$_toolu_lib/diff-sha.sh"
# shellcheck source=../../lib/telemetry.sh
. "$_toolu_lib/telemetry.sh"

# Degrade silent on anything that isn't a real push in a real branch.
[[ "$tool_name" != "Bash" ]] && exit 0
command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')
is_git_push "$command" || exit 0

current_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
# Detached HEAD or no branch — nothing to slug/key on; stay silent.
[[ -z "$current_branch" || "$current_branch" == "HEAD" ]] && exit 0

base_branch="${DOCS_SYNC_BASE:-$(detect_base_branch)}"
# Missing base or pushing the base itself: no feature diff to judge.
git rev-parse --verify --quiet "$base_branch" >/dev/null 2>&1 || exit 0
[[ "$current_branch" == "$base_branch" ]] && exit 0

# Changed paths drive surface matching; an empty list means nothing to nudge.
changed=$(git diff --name-only "${base_branch}...HEAD" 2>/dev/null || echo "")
[[ -z "$changed" ]] && exit 0

# Content-addressed diff sha (mirrors push-review.sh) keys the attestation, so
# changing the code invalidates a stale "not-needed" attestation.
diff_sha=$(toolu_diff_sha . "$base_branch")

# Classify the changed paths against the configurable glob sets. `case` fnmatch
# treats a single `*` as crossing `/`, so `docs/*.md` covers nested paths.
surfaces=$(docs_sync_surfaces)
surface_excludes=$(docs_sync_surface_excludes)
code_surfaces=$(docs_sync_code_surfaces)

_matches_any() {
  local path="$1" glob
  while IFS= read -r glob; do
    [[ -z "$glob" ]] && continue
    # shellcheck disable=SC2254  # $glob is an intentional fnmatch pattern.
    case "$path" in
      $glob) return 0 ;;
    esac
  done <<< "$2"
  return 1
}

has_doc=0 has_code=0
while IFS= read -r path; do
  [[ -z "$path" ]] && continue
  # A doc hit is a surface match that is NOT carved out by an exclude (e.g.
  # release notes, which `docs/*.md` would otherwise swallow since `*` crosses `/`).
  if _matches_any "$path" "$surfaces" && ! _matches_any "$path" "$surface_excludes"; then
    has_doc=1
  fi
  if _matches_any "$path" "$code_surfaces"; then has_code=1; fi
done <<< "$changed"

# Nudge only when code changed AND no doc surface did.
{ [[ "$has_code" == 1 && "$has_doc" == 0 ]]; } || exit 0

# off must be indistinguishable from the module not existing: no telemetry
# (not even docs_attested for an otherwise-valid attestation below), no
# output. Read here (rather than at top-of-file) so an off branch still pays
# no cost beyond the code/doc classification already needed either way.
mode=$(toolu_string docsSync.mode advise advise block off)
[[ "$mode" == "off" ]] && exit 0

# Attestation gate: a fresh (diff-sha-matching) attestation silences the nudge
# (in both advise and block mode).
slug=$(branch_slug "$current_branch")
state_dir=${DOCS_SYNC_STATE_DIR:-${CLAUDE_PROJECT_DIR:-$(pwd)}/.claude/tmp/docs-sync}
state_file="$state_dir/${slug}.json"
if [[ -f "$state_file" && -n "$diff_sha" ]]; then
  attested_sha=$(jq -r '.diff_sha // ""' "$state_file" 2>/dev/null || echo "")
  if [[ "$attested_sha" == "$diff_sha" ]]; then
    attested_decision=$(jq -r '.decision // ""' "$state_file" 2>/dev/null || echo "")
    telemetry_append "$(detect_project_root)" "docs_attested" \
      "$(jq -cn --arg d "$attested_decision" '{decision: $d}')"
    exit 0
  fi
fi

telemetry_append "$(detect_project_root)" "docs_nudge"

# Shared instructions (the attestation escape hatch) for both modes; only the
# closing sentence and the JSON shape (additionalContext vs. deny) differ.
jq -n --arg sha "$diff_sha" --arg file "$state_file" --arg mode "$mode" '
  ("docs-sync: this branch changes code but no documentation surface " +
   "(README / docs/*.md / SKILL.md). Update the doc that describes this " +
   "behavior, OR attest none is needed by writing " + $file + " with " +
   "{ \"version\": 1, \"diff_sha\": \"" + $sha + "\", \"decision\": \"not-needed\", \"note\": \"why\" }. " +
   (if $mode == "block"
    then "Push denied until a doc is updated or a valid attestation is written."
    else "Advisory only — this does not block the push."
    end)
  ) as $reason
  | if $mode == "block" then {
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": $reason
      }
    } else {
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "additionalContext": $reason
      }
    } end
'
exit 0
