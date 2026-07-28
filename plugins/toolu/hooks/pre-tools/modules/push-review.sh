#!/usr/bin/env bash
# Pre-tool check: block `git push` until branch has been reviewed.
# Project-agnostic: base branch is detected via detect_base_branch
# (or env-overridden with $PUSH_REVIEW_BASE for tests).
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload (also piped to stdin; this module reads the env var)
#
# State file: .claude/tmp/push-review/<branch-slug>.json
# Override via $STATE_DIR for testing.

# pipefail so a `git diff | git hash-object` failure surfaces; without it,
# an empty diff stream succeeds and yields the well-known empty-blob SHA,
# which a stale state file can cache and reuse across any future empty-diff
# state on the same branch (including post-force-reset).
set -o pipefail

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"

[[ "$tool_name" != "Bash" ]] && exit 0

command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')

# Push detection (strip_heredocs + boundary-anchored regex) is shared via
# detect.sh's is_git_push. The trailing boundary also catches statement
# terminators (`git push;`, `git push&`, `git push|tee`) — without them an agent
# could append `;` and slip the push past the gate, which is now the only
# push-time check.
is_git_push "$command" || exit 0

# Every check below reads the repo the push TARGETS, not the hook's cwd — a
# `git -C <worktree> push` must be judged on the worktree's branch, diff and
# state file. Keying off $CLAUDE_PROJECT_DIR (a main-checkout path) split the
# state file in two: the reviewer wrote the worktree's copy while the gate read
# the main checkout's, so no amount of re-reviewing could satisfy it.
repo_root=$(push_target_root "$command")

current_branch=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
slug=$(branch_slug "$current_branch")

# Resolve state dir: env override takes precedence; else target-repo default.
state_dir=${STATE_DIR:-$repo_root/.claude/tmp/push-review}
state_file="$state_dir/${slug}.json"

# Base branch: env override > detect_base_branch, resolved in the target repo.
base_branch="${PUSH_REVIEW_BASE:-$(detect_base_branch "$repo_root")}"

# Verify base branch exists locally.
if ! git -C "$repo_root" rev-parse --verify --quiet "$base_branch" >/dev/null; then
  jq -n --arg base "$base_branch" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("base branch '\''" + $base + "'\'' not found locally; run `git fetch origin " + $base + ":" + $base + "`")
    }
  }'
  exit 0
fi

# Detect detached HEAD (current_branch == "HEAD" from rev-parse).
if [[ "$current_branch" == "HEAD" || -z "$current_branch" ]]; then
  jq -n '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": "detached HEAD — checkout a branch before push"
    }
  }'
  exit 0
fi

# Pushing the base branch itself (e.g. fast-forwarded main after a local merge)
# is not a feature-review scenario — `<base>...<base>` has empty diff by
# definition. Allow without state-file gate.
if [[ "$current_branch" == "$base_branch" ]]; then
  exit 0
fi

# Compute branch diff SHA (content-addressed, survives amend/rebase).
# Well-known git empty-blob SHA — the value `git hash-object --stdin` produces
# for an empty stream. We refuse to cache or trust this value: a state file
# bearing it would be reusable across any future empty-diff state on the same
# branch (e.g. after a force reset wipes commits).
EMPTY_BLOB_SHA="e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"

current_diff_sha=$(git -C "$repo_root" diff --no-color "${base_branch}...HEAD" 2>/dev/null | git -C "$repo_root" hash-object --stdin 2>/dev/null || echo "")
if [[ -z "$current_diff_sha" ]]; then
  # git diff failed (disk full, etc). Allow push; underlying push will surface real failure.
  echo "push-review: git diff ${base_branch}...HEAD failed; allowing push to surface real error" >&2
  exit 0
fi

if [[ "$current_diff_sha" == "$EMPTY_BLOB_SHA" ]]; then
  # Empty diff against base: nothing to review and nothing to push. Treat as
  # a sentinel that NEVER satisfies the cache check, then deny the push so
  # the user is forced to confirm intent rather than push a no-op.
  current_diff_sha="empty-diff"
  jq -n --arg base "$base_branch" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "Refusing to push: diff against " + $base + " is empty. " +
        "Either no commits diverged from base, or the branch was force-reset. " +
        "Verify intent before pushing."
      )
    }
  }'
  exit 0
fi

# Reviewer guidance is agnostic: any one accepted reviewer satisfies the gate.
# Prefer caveman's cavecrew-reviewer when that plugin is installed; otherwise the
# built-in /code-review skill is the always-available baseline.
if [ -n "$(detect_plugin_installed 'caveman@caveman' 2>/dev/null)" ]; then
  reviewer_hint="\`caveman:cavecrew-reviewer\` (caveman is installed — preferred), recorded as \"caveman:cavecrew-reviewer\""
else
  reviewer_hint="the built-in \`/code-review xhigh --fix\` skill, recorded as \"code-review\" (or the \`toolu-review:review\` skill, or install the caveman plugin and use \`caveman:cavecrew-reviewer\`)"
fi

# State file gate.
if [[ ! -f "$state_file" ]]; then
  jq -n --arg sha "$current_diff_sha" --arg base "$base_branch" --arg file "$state_file" --arg hint "$reviewer_hint" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "Code review required before push (diff SHA " + $sha + ", base " + $base + ").\n" +
        "Run a code reviewer on `git diff " + $base + "...HEAD` and apply its findings — use " + $hint + ". " +
        "Then atomically write " + $file + " (tmp+mv) with schema " +
        "{ version: 1, branch, diff_sha, base_branch, reviewed_at, reviewers, findings_count, findings, review_round }. " +
        "`reviewers` must include at least one accepted reviewer (caveman:cavecrew-reviewer, code-review, toolu-review:review, code-review:xhigh, review, or security-review), " +
        "`findings_count` must be 0, `review_round` starts at 1 for a new `diff_sha` and bumps by 1 only when rewriting at the same `diff_sha`. " +
        "Retry push."
      )
    }
  }'
  exit 0
fi

# Validate state file: version, diff_sha, findings_count, reviewers.
state_version=$(jq -r '.version // ""' "$state_file" 2>/dev/null || echo "")
state_sha=$(jq -r '.diff_sha // ""' "$state_file" 2>/dev/null || echo "")
state_findings=$(jq -r '.findings_count // ""' "$state_file" 2>/dev/null || echo "")

if [[ "$state_version" != "1" || -z "$state_sha" || -z "$state_findings" ]]; then
  jq -n --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("state file corrupted at " + $file + "; delete and re-review")
    }
  }'
  exit 0
fi

# Reviewer-agnostic gate: at least ONE accepted reviewer must appear in the
# state file. This keeps toolu usable without the caveman plugin — the
# built-in /code-review skill is the always-available baseline — while still
# accepting caveman:cavecrew-reviewer (preferred when installed) and other known
# reviewers. The check is "intersection non-empty", not equality, so running
# extra reviewers (e.g. code-simplifier first) is always fine. Requiring at
# least one known name still prevents an agent from writing a junk reviewer
# entry to bypass the gate.
accepted_reviewers='["caveman:cavecrew-reviewer","code-review","toolu-review:review","code-review:xhigh","review","security-review"]'
if ! jq -e --argjson acc "$accepted_reviewers" \
     '(.reviewers // []) as $r | any($acc[]; . as $x | $r | index($x) != null)' \
     "$state_file" >/dev/null 2>&1; then
  jq -n --arg file "$state_file" --arg acc "$accepted_reviewers" --arg hint "$reviewer_hint" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "state file lists no accepted reviewer at " + $file + "\n" +
        "`reviewers` must include at least one of: " + $acc + "\n" +
        "Run a reviewer — use " + $hint + " — then rewrite the state file."
      )
    }
  }'
  exit 0
fi

# Max review rounds — bound the fix→re-review loop. `review_round` counts
# rewrites AT THE SAME diff_sha: a changed diff resets it to 1, so the counter
# measures "reviewers keep finding issues in code that never changed", not
# "this branch has been worked on for a while". Counting every rewrite instead
# made the cap terminal — nothing resets it after a push, so a long-lived
# branch eventually denied every push forever.
# Missing field is treated as round 1 for backward compat.
# After MAX_ROUNDS, deny with an escalation message so the babysit triggers
# its Step 6 escalation stop instead of looping indefinitely.
MAX_ROUNDS=5
state_round=$(jq -r '.review_round // 1' "$state_file" 2>/dev/null || echo "1")
if [[ "$state_round" =~ ^[0-9]+$ ]] && (( state_round > MAX_ROUNDS )); then
  jq -n --arg n "$state_round" --arg max "$MAX_ROUNDS" --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "ESCALATE: review loop hit " + $n + " rounds (max " + $max + ") on an unchanged diff at " + $file + ". " +
        "Reviewers keep finding new issues after each fix — stop auto-looping and surface the " +
        "current findings to the human. Babysit: treat as Escalation stop (Step 6)."
      )
    }
  }'
  exit 0
fi

if [[ "$state_sha" != "$current_diff_sha" ]]; then
  jq -n --arg sha "$current_diff_sha" --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "Code review required: diff changed since review.\n" +
        "Current diff SHA: " + $sha + "\n" +
        "State file: " + $file + " (stale)\n" +
        "Re-run reviewers on the new diff and rewrite the state file."
      )
    }
  }'
  exit 0
fi

if [[ "$state_findings" != "0" ]]; then
  jq -n --arg count "$state_findings" --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "Code review has open findings (" + $count + ").\n" +
        "State file: " + $file + "\n" +
        "Address every finding (any finding blocks). Re-commit. Re-run reviewers. Rewrite state file with findings_count=0."
      )
    }
  }'
  exit 0
fi

# All gates pass: allow push.
exit 0
