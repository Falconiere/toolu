#!/usr/bin/env bash
# Post-tool: turn "the user said yes" into a durable push-review waiver.
#
# push-review.sh (PreToolUse) can only ask; it never learns the answer. This
# module is the other half: PostToolUse runs only when the tool actually ran,
# so its presence IS the yes. It promotes the pending marker push-review left
# behind into a waiver for that exact diff.
#
# Two guards keep a waiver honest:
#   * the diff SHA must still match what was asked about — a pending marker
#     from older code is never cashed in by a push of different code;
#   * the push must have SUCCEEDED. PostToolUse fires when a tool ran, not when
#     it worked, and a rejected non-fast-forward push has not been reviewed by
#     anybody. On failure the pending marker survives for the retry.
#
# Inputs (from the parent dispatcher post-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

# pipefail for parity with the other gate modules (push-review.sh,
# plan-ledger.sh, docs-sync.sh all use exactly this).
#
# Deliberately NOT -e and NOT -u, for the same underlying reason: both options
# apply to the shared libraries this module SOURCES, which are written for the
# shell they are sourced into rather than for a strict one.
#
#   -e: this module's control flow uses non-zero returns as signal —
#       push_waiver_promote returns 1 whenever there is nothing to promote,
#       which is the normal case on most pushes.
#   -u: the libraries use the guard-then-assign idiom, reading a variable
#       before it is ever assigned. hooks/lib/detect.sh:307 is one of several:
#           [ -n "$root" ] || root=$(git rev-parse --show-toplevel 2>/dev/null)
#       Under nounset that aborts with "root: unbound variable" before this
#       module reaches its own work, so a real push silently fails to record
#       its waiver. Verified: adding -u fails
#       __tests__/push-waiver.bats "the module never writes to stdout".
#
# Making these strict is a repo-wide change to how the shared libraries are
# written, not a per-module flag.
set -o pipefail

: "${tool_name:=}"
: "${input:=}"

# Cursor Agent uses tool_name "Shell"; Claude Code uses "Bash".
[[ "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0

command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/diff-sha.sh
. "$_toolu_lib/diff-sha.sh"
# shellcheck source=../../lib/push-waiver.sh
. "$_toolu_lib/push-waiver.sh"

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null || echo "")
is_git_push "$command" || exit 0

# Exit-code resolution mirrors post-tools/modules/gate-status.sh so both
# modules read a host's response shape the same way.
exit_code=$(echo "$input" | jq -r '.tool_response.metadata.exit_code // .tool_response.exit_code // .tool_output.exit_code // empty' 2>/dev/null || echo "")
if [[ -z "$exit_code" || "$exit_code" == "null" ]]; then
  tool_output_raw=$(echo "$input" | jq -r '.tool_output // empty' 2>/dev/null || echo "")
  if [[ -n "$tool_output_raw" ]]; then
    exit_code=$(echo "$tool_output_raw" | jq -r '.exitCode // .exit_code // empty' 2>/dev/null || echo "")
  fi
fi

# A reported non-zero exit means the push did not land: leave the pending
# marker alone so the retry still asks.
if [[ -n "$exit_code" && "$exit_code" != "null" && "$exit_code" != "0" ]]; then
  exit 0
fi

# An interrupted tool call did not complete either.
interrupted=$(echo "$input" | jq -r '.tool_response.interrupted // false' 2>/dev/null || echo "false")
[[ "$interrupted" == "true" ]] && exit 0

# Same target-repo resolution as the pre-tool gate: `git -C <worktree> push`
# is judged on the worktree's own branch and state.
repo_root=$(push_target_root "$command")
current_branch=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
[[ -z "$current_branch" || "$current_branch" == "HEAD" ]] && exit 0
slug=$(branch_slug "$current_branch")

base_branch="${PUSH_REVIEW_BASE:-$(detect_base_branch "$repo_root")}"
current_diff_sha=$(toolu_diff_sha "$repo_root" "$base_branch")
[[ -z "$current_diff_sha" ]] && exit 0

# Nothing pending, or pending for other code: promote is a no-op and says so
# by returning non-zero. Either way this module has no output.
push_waiver_promote "$repo_root" "$slug" "$current_diff_sha" || exit 0

exit 0
