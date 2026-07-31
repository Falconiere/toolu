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
# shellcheck source=../../lib/diff-sha.sh
. "$_toolu_lib/diff-sha.sh"
# shellcheck source=../../lib/telemetry.sh
. "$_toolu_lib/telemetry.sh"

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

# push_check telemetry: one closed set of reason codes across every decision
# exit below (base-missing, detached-head, diff-failed, no-state, stale-diff,
# schema-v1, schema, file-coverage, findings, reviewer, round-cap, empty-diff,
# pass). detached-head is unreachable-by-contract in the stream: telemetry_append
# refuses branch=="HEAD", so that deny never produces a line — the call stays for
# consistency should the contract change. ROUND is the state file's review_round
# when it's already known at the call site; the exits that precede reading the
# state file (or that fail its schema) pass "" -> JSON null.
_pr_telemetry() {
  local result="$1" code="$2" round="${3:-}"
  local round_json="null"
  [[ "$round" =~ ^[0-9]+$ ]] && round_json="$round"
  telemetry_append "$repo_root" "push_check" \
    "$(jq -cn --arg result "$result" --arg code "$code" --argjson round "$round_json" \
         '{result: $result, reason_code: $code, round: $round}')"
}

# Verify base branch exists locally.
if ! git -C "$repo_root" rev-parse --verify --quiet "$base_branch" >/dev/null; then
  jq -n --arg base "$base_branch" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("base branch '\''" + $base + "'\'' not found locally; run `git fetch origin " + $base + ":" + $base + "`")
    }
  }'
  _pr_telemetry deny base-missing
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
  _pr_telemetry deny detached-head
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

current_diff_sha=$(toolu_diff_sha "$repo_root" "$base_branch")
if [[ -z "$current_diff_sha" ]]; then
  # git diff failed (disk full, etc). Allow push; underlying push will surface real failure.
  echo "push-review: git diff ${base_branch}...HEAD failed; allowing push to surface real error" >&2
  _pr_telemetry allow diff-failed
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
  _pr_telemetry deny empty-diff
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
        "{ version: 2, branch, diff_sha, base_branch, reviewed_at, reviewers, findings_count, findings, review_round, reviewed_files }. " +
        "`reviewers` must include at least one accepted reviewer (caveman:cavecrew-reviewer, code-review, toolu-review:review, code-review:xhigh, review, or security-review), " +
        "`findings_count` must be 0, `review_round` starts at 1 for a new `diff_sha` and bumps by 1 only when rewriting at the same `diff_sha`. " +
        "`reviewed_files` must list every path from `git diff " + $base + "...HEAD --name-only` (sorted, unique) — the actual reviewer file coverage. " +
        "Retry push."
      )
    }
  }'
  _pr_telemetry deny no-state
  exit 0
fi

# Validate state file: version, diff_sha, findings_count, reviewers.
state_version=$(jq -r '.version // ""' "$state_file" 2>/dev/null || echo "")
state_sha=$(jq -r '.diff_sha // ""' "$state_file" 2>/dev/null || echo "")
state_findings=$(jq -r '.findings_count // ""' "$state_file" 2>/dev/null || echo "")

# A schema v1 state (no reviewed_files contract) gets a dedicated one-time
# upgrade deny rather than the generic "corrupted" deny — the state is valid,
# just stale-schema, and the fix is a re-review, not a delete-and-retry.
if [[ "$state_version" == "1" ]]; then
  jq -n '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file"
    }
  }'
  _pr_telemetry deny schema-v1
  exit 0
fi

if [[ "$state_version" != "2" || -z "$state_sha" || -z "$state_findings" ]]; then
  jq -n --arg file "$state_file" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("state file corrupted at " + $file + "; delete and re-review")
    }
  }'
  _pr_telemetry deny schema
  exit 0
fi

# review_round: read here (once) rather than only at the round-cap check below,
# so it's already known for the reviewer-acceptance telemetry exit too. Missing
# field defaults to round 1 for backward compat (same default the round-cap
# check uses).
state_round=$(jq -r '.review_round // 1' "$state_file" 2>/dev/null || echo "1")

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
  _pr_telemetry deny reviewer "$state_round"
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
  _pr_telemetry deny round-cap "$state_round"
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
  _pr_telemetry deny stale-diff "$state_round"
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
  _pr_telemetry deny findings "$state_round"
  exit 0
fi

# Reviewer file coverage (v2): reviewed_files must equal the diff's changed
# paths (sorted, unique) — catches honestly-reported partial review scope.
# Not adversary-proof against a writer that lies about what it reviewed
# (Non-Goal 3, spec). A missing/malformed reviewed_files reads as empty here
# (jq errors are silenced), which correctly denies naming every changed path
# as missing rather than passing vacuously.
changed_sorted=$(git -C "$repo_root" diff --no-color "${base_branch}...HEAD" --name-only 2>/dev/null | sort -u)
reviewed_sorted=$(jq -r '.reviewed_files[]' "$state_file" 2>/dev/null | sort -u)

if [[ "$changed_sorted" != "$reviewed_sorted" ]]; then
  missing=$(comm -23 <(printf '%s\n' "$changed_sorted") <(printf '%s\n' "$reviewed_sorted"))
  extra=$(comm -13 <(printf '%s\n' "$changed_sorted") <(printf '%s\n' "$reviewed_sorted"))
  jq -n --arg file "$state_file" --arg missing "$missing" --arg extra "$extra" --arg base "$base_branch" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": (
        "reviewed_files does not match the current diff at " + $file + ".\n" +
        (if ($missing | length) > 0 then "Missing from reviewed_files (changed but not reviewed): " + $missing + "\n" else "" end) +
        (if ($extra | length) > 0 then "In reviewed_files but not in the current diff: " + $extra + "\n" else "" end) +
        "Re-review the full diff and rewrite reviewed_files to match `git diff " + $base + "...HEAD --name-only` exactly."
      )
    }
  }'
  _pr_telemetry deny file-coverage "$state_round"
  exit 0
fi

# All gates pass: allow push.
_pr_telemetry allow pass "$state_round"
exit 0
