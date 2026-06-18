#!/usr/bin/env bats
# Tests for .claude/hooks/pre-tools/modules/push-review.sh

load helpers

setup() {
  setup_sandbox
}

teardown() {
  teardown_sandbox
}

@test "push-review: non-Bash tool exits silently" {
  payload=$(build_input "git push")
  run_hook "Edit" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: Bash but not git push exits silently" {
  payload=$(build_input "ls -la")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: git push inside heredoc body is ignored" {
  payload=$(build_input 'git commit -m "$(cat <<EOF
about git push
EOF
)"')
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: branch slug strips slashes" {
  git checkout -q -b feat/x/y
  echo a > a.txt && git add a.txt && git commit -q -m a
  payload=$(build_input "git push")
  STATE_DIR="$STATE_DIR" run_hook "Bash" "$payload"
  # State file path that the hook would write/read:
  [ -d "$STATE_DIR" ]
  # Branch slug check is indirect — once the hook denies (Task 5),
  # the reason string will name the file.
  # For now, drive the slug function directly:
  run bash -c '
    branch="feat/x/y"
    echo "$branch" | tr "/" "_" | tr -cd "a-zA-Z0-9_-"
  '
  [ "$status" -eq 0 ]
  [ "$output" = "feat_x_y" ]
}

@test "push-review: empty slug falls back to _default" {
  run bash -c '
    branch=""
    slug=$(echo "$branch" | tr "/" "_" | tr -cd "a-zA-Z0-9_-")
    [[ -z "$slug" ]] && slug="_default"
    echo "$slug"
  '
  [ "$status" -eq 0 ]
  [ "$output" = "_default" ]
}

@test "push-review: diff SHA is stable for unchanged content" {
  sha1=$(current_diff_sha)
  sha2=$(current_diff_sha)
  [ -n "$sha1" ]
  [ "$sha1" = "$sha2" ]
}

@test "push-review: diff SHA changes when content changes" {
  sha1=$(current_diff_sha)
  echo "more" >> feature.txt
  git commit -q -am "more"
  sha2=$(current_diff_sha)
  [ "$sha1" != "$sha2" ]
}

@test "push-review: diff SHA survives commit --amend with identical content" {
  sha1=$(current_diff_sha)
  git commit -q --amend --no-edit
  sha2=$(current_diff_sha)
  [ "$sha1" = "$sha2" ]
}

@test "push-review: git push with no state file is denied" {
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("Code review required")'
}

@test "push-review: git push with matching SHA and zero findings is allowed" {
  sha=$(current_diff_sha)
  write_state "$sha" 0
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: git push with matching SHA and open findings is denied" {
  sha=$(current_diff_sha)
  write_state "$sha" 3
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("open findings")'
}

@test "push-review: git push with stale SHA is denied" {
  write_state "stale_sha_value_0000000000000000000000" 0
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("diff changed since review")'
}

@test "push-review: corrupted state file is denied" {
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  echo "not json" > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("corrupted")'
}

@test "push-review: state file missing required keys is denied" {
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  echo '{"version": 1}' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("corrupted")'
}

@test "push-review: state file with wrong version is denied" {
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n '{version: 2, diff_sha: "x", findings_count: 0}' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("corrupted")'
}

@test "push-review: missing base branch is denied with fetch hint" {
  git branch -q -D development 2>/dev/null || true
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("base branch")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("git fetch")'
}

@test "push-review: detached HEAD is denied" {
  sha=$(git rev-parse HEAD)
  git checkout -q "$sha"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("detached HEAD")'
}

@test "push-review: denial reason instructs agent to use atomic write" {
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("atomic")'
}

@test "push-review: dispatcher picks up the module by glob" {
  # The dispatcher reads `*.sh` from modules/. Verify our file matches.
  shopt -s nullglob
  modules=("$REPO_ROOT"/hooks/pre-tools/modules/*.sh)
  found=0
  for m in "${modules[@]}"; do
    [[ "$m" == *push-review.sh ]] && found=1
  done
  [ "$found" -eq 1 ]
}

# Regression: well-known empty-blob SHA must never be trusted as cached state.
# A branch with no diff against base would otherwise be pushable via a stale
# state file written before a force-reset/clean.
@test "push-review: empty diff against base is denied with sentinel reason (even with matching state file)" {
  # Move HEAD back to base so the diff is empty.
  git checkout -q development
  git checkout -q -b feat/empty
  # No commits diverged from base.

  # Pre-write a state file claiming the empty-blob SHA with zero findings.
  EMPTY_BLOB="e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
  write_state "$EMPTY_BLOB" 0

  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("diff against")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("empty")'
}

@test "push-review: pushing the base branch itself is ALLOWED (no state file required)" {
  # Switch to the base branch (development in the sandbox); diff against
  # itself is empty, but we MUST NOT deny — this is the legitimate
  # "fast-forward and push the integration branch" flow.
  git checkout -q development

  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: non-empty diff with matching SHA + zero findings still ALLOWED" {
  # This is the happy-path regression: the sentinel guard must not break it.
  sha=$(current_diff_sha)
  write_state "$sha" 0
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: state file with no accepted reviewer is denied" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["simplify"],
    findings_count: 0,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("no accepted reviewer")'
}

@test "push-review: the built-in code-review reviewer alone is allowed (agnostic baseline)" {
  sha=$(current_diff_sha)
  write_state "$sha" 0
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: toolu-review:review reviewer alone satisfies the accepted set" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["toolu-review:review"],
    findings_count: 0,
    review_round: 1,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: caveman reviewer alone satisfies the accepted set" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["caveman:cavecrew-reviewer"],
    findings_count: 0,
    review_round: 1,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: code-simplifier alone (not an accepted reviewer) is denied" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier"],
    findings_count: 0,
    review_round: 1,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("no accepted reviewer")'
}

@test "push-review: extra reviewers beyond the accepted set still pass (superset)" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier", "caveman:cavecrew-reviewer"],
    findings_count: 0,
    review_round: 1,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: review_round above MAX_ROUNDS triggers escalation deny" {
  sha=$(current_diff_sha)
  write_state "$sha" 0 6
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("ESCALATE")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("Escalation stop")'
}

@test "push-review: review_round at MAX_ROUNDS still allowed" {
  sha=$(current_diff_sha)
  write_state "$sha" 0 5
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: missing review_round defaults to 1 (backward compat)" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier", "caveman:cavecrew-reviewer"],
    findings_count: 0,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: full loop — first push denied, fix loop, final push allowed" {
  # 1. First push: no state file → DENY.
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'

  # 2. Agent writes state file with findings.
  sha1=$(current_diff_sha)
  write_state "$sha1" 2

  # 3. Second push: open findings → DENY.
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("open findings")'

  # 4. Agent fixes a finding via a new commit; SHA changes.
  echo "fix" >> feature.txt
  git commit -q -am "fix finding"
  sha2=$(current_diff_sha)
  [ "$sha1" != "$sha2" ]

  # 5. Third push (no rewrite yet): stale SHA → DENY.
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("diff changed since review")'

  # 6. Agent re-reviews, writes clean state file with new SHA.
  write_state "$sha2" 0

  # 7. Fourth push: clean → ALLOW.
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
