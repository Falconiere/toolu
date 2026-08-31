#!/usr/bin/env bats
# Tests for .claude/hooks/pre-tools/modules/push-review.sh

load helpers

setup() {
  setup_sandbox
  # These cases were written against the original always-deny gate; that is
  # now the `strict` preset. The balanced/advise default and the opt-in ask
  # path have their own block at the end of this file.
  use_strict_preset
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
  echo '{"version": 2}' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("corrupted")'
}

@test "push-review: state file with wrong version is denied" {
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n '{version: 3, diff_sha: "x", findings_count: 0}' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("corrupted")'
}

@test "push-review: state file with schema v1 is denied with the dedicated upgrade message" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 1,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-review"],
    findings_count: 0,
    review_round: 1,
    findings: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason ==
    "push-review state is schema v1; harness v2 requires reviewed_files — re-run the review to regenerate the state file"'
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["simplify"],
    findings_count: 0,
    findings: [],
    reviewed_files: ["feature.txt"]
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["toolu-review:review"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt"]
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["caveman:cavecrew-reviewer"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt"]
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt"]
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier", "caveman:cavecrew-reviewer"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt"]
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
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-simplifier", "caveman:cavecrew-reviewer"],
    findings_count: 0,
    findings: [],
    reviewed_files: ["feature.txt"]
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

# --- reviewer file coverage (push-review v2) ----------------------------

@test "push-review: reviewed_files missing one changed file is denied naming the path" {
  echo "second" > second.txt
  git add second.txt
  git commit -q -m "second file"
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-review"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt"]
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("reviewed_files does not match")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("second.txt")'
}

@test "push-review: reviewed_files with the full changed-file list is allowed" {
  echo "second" > second.txt
  git add second.txt
  git commit -q -m "second file"
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-review"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt", "second.txt"]
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: reviewed_files with an extra path not in the diff is denied naming it" {
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 2,
    branch: "feat/example",
    diff_sha: $sha,
    base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z",
    reviewers: ["code-review"],
    findings_count: 0,
    review_round: 1,
    findings: [],
    reviewed_files: ["feature.txt", "nonexistent.txt"]
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("reviewed_files does not match")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("nonexistent.txt")'
}

# --- worktree targeting -------------------------------------------------
#
# pr-babysit pushes with `git -C <worktree> push` from a session rooted in the
# main checkout. These exercise the default (non-$STATE_DIR) resolution, since
# the bug was precisely that the gate read the wrong repo's state.

# Run the hook with $STATE_DIR unset so the default state-dir resolution runs.
run_hook_default_statedir() {
  local payload="$1"
  run env -u STATE_DIR -u CLAUDE_PROJECT_DIR \
    tool_name=Bash input="$payload" PUSH_REVIEW_BASE=development \
    bash "$HOOK_SCRIPT" <<<"$payload"
}

# Write a state file under an arbitrary repo root's default state dir.
# reviewed_files is computed from the real diff at $root (always feature.txt
# across these worktree fixtures — the worktree never gains a second commit).
write_state_in() {
  local root="$1" branch="$2" sha="$3" count="$4"
  local slug
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  mkdir -p "$root/.claude/tmp/push-review"
  local reviewed_files
  reviewed_files=$(git -C "$root" diff --no-color "development...HEAD" --name-only \
    | jq -R -s -c 'split("\n") | map(select(length > 0))')
  jq -n --arg branch "$branch" --arg sha "$sha" --argjson count "$count" --argjson reviewed_files "$reviewed_files" \
    '{version:2, branch:$branch, diff_sha:$sha, base_branch:"development",
      reviewed_at:"2026-07-28T00:00:00Z", reviewers:["code-review"],
      findings_count:$count, review_round:1, findings:[], reviewed_files:$reviewed_files}' \
    > "$root/.claude/tmp/push-review/${slug}.json"
}

setup_worktree() {
  git checkout -q development
  git worktree add -q "$SANDBOX/wt" feat/example
  WT_REAL=$(cd "$SANDBOX/wt" && pwd -P)
  WT_SHA=$(git -C "$SANDBOX/wt" diff --no-color development...HEAD | git hash-object --stdin)
}

# Move the main checkout onto a diverging branch that has no state file, so a
# gate reading its own cwd would deny. Lets an "allow" assertion prove the gate
# actually read the worktree rather than fell through the base-branch exemption.
diverge_main_checkout() {
  git checkout -q -b other development
  echo other > other.txt
  git add other.txt
  git commit -q -m "other commit"
}

@test "push-review: git -C <worktree> push is gated on the worktree, not the cwd" {
  setup_worktree
  # The main checkout sits on the base branch. Before the fix the gate read its
  # own cwd, matched current_branch == base_branch, and allowed the push
  # unreviewed — a silent bypass on every babysit worktree push.
  payload=$(build_input "git -C $SANDBOX/wt push")
  run_hook_default_statedir "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("Code review required before push")'
}

@test "push-review: a state file in the main checkout does not authorize a worktree push" {
  setup_worktree
  # Correct SHA, correct branch, wrong repo root — the reviewer wrote it where
  # the gate used to look. It must not satisfy the worktree's push.
  write_state_in "$SANDBOX" "feat/example" "$WT_SHA" 0
  payload=$(build_input "git -C $SANDBOX/wt push")
  run_hook_default_statedir "$payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e --arg wt "$WT_REAL" \
    '.hookSpecificOutput.permissionDecisionReason | test($wt)'
}

@test "push-review: worktree push allowed once the worktree's own state is clean" {
  setup_worktree
  # Park the main checkout on a diverging branch with no state file, so an
  # allow here can only come from reading the worktree — a gate that consulted
  # its own cwd would deny on `other`'s missing state.
  diverge_main_checkout
  payload=$(build_input "git -C $SANDBOX/wt push")

  # Engaged: silence at this point would mean the push was never gated at all
  # (the old is_git_push did not match `git -C <path> push`).
  run_hook_default_statedir "$payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'

  write_state_in "$WT_REAL" "feat/example" "$WT_SHA" 0
  run_hook_default_statedir "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: worktree push still denies on open findings" {
  setup_worktree
  write_state_in "$WT_REAL" "feat/example" "$WT_SHA" 2
  payload=$(build_input "git -C $SANDBOX/wt push")
  run_hook_default_statedir "$payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("open findings")'
}

@test "push-review: \$CLAUDE_PROJECT_DIR no longer decides where state is read" {
  setup_worktree
  diverge_main_checkout
  elsewhere=$(mktemp -d)
  payload=$(build_input "git -C $SANDBOX/wt push")

  run env -u STATE_DIR CLAUDE_PROJECT_DIR="$elsewhere" \
    tool_name=Bash input="$payload" PUSH_REVIEW_BASE=development \
    bash "$HOOK_SCRIPT" <<<"$payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'

  write_state_in "$WT_REAL" "feat/example" "$WT_SHA" 0
  run env -u STATE_DIR CLAUDE_PROJECT_DIR="$elsewhere" \
    tool_name=Bash input="$payload" PUSH_REVIEW_BASE=development \
    bash "$HOOK_SCRIPT" <<<"$payload"
  rm -rf "$elsewhere"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# ── Modes: the shipped default and its neighbours ───────────────────────────
#
# Everything above pins `strict`. These cases exercise what a user actually
# gets out of the box (balanced -> advise), the opt-in ask/waiver path, and
# the two escape hatches.

# Promote a pending waiver the way post-tools/modules/push-waiver.sh does,
# using the real lib rather than a hand-written file.
promote_waiver() {
  local sha="$1"
  STATE_DIR="$STATE_DIR" bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/push-waiver.sh"
    push_waiver_promote "'"$SANDBOX"'" feat_example "'"$sha"'"
  '
}

@test "push-review: default preset advises instead of denying" {
  gate_config '{"version":1}'
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" == *"Code review required"* ]]
}

@test "push-review: default advise does not record a pending waiver" {
  gate_config '{"version":1}'
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ ! -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "push-review: ask mode keeps the reviewer instructions" {
  use_ask_mode
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"Code review required before push"* ]]
  [[ "$reason" == *"remembered until the diff changes"* ]]
}

@test "push-review: asking records a pending waiver for the current diff" {
  use_ask_mode
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  sha=$(current_diff_sha)
  [ -f "$STATE_DIR/feat_example.pending-waiver.json" ]
  [ "$(jq -r '.diff_sha' "$STATE_DIR/feat_example.pending-waiver.json")" = "$sha" ]
  [ "$(jq -r '.reason_code' "$STATE_DIR/feat_example.pending-waiver.json")" = "no-state" ]
}

@test "push-review: a promoted waiver lets the same diff through silently" {
  use_ask_mode
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  promote_waiver "$(current_diff_sha)"
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: a new commit invalidates the waiver and asks again" {
  use_ask_mode
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  promote_waiver "$(current_diff_sha)"
  echo "more" > more.txt && git add more.txt && git commit -q -m "feat: more"
  run_hook "Bash" "$payload"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
}

@test "push-review: a waiver does not cover a different branch" {
  use_ask_mode
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  promote_waiver "$(current_diff_sha)"
  git checkout -q -b feat/other
  echo "other" > other.txt && git add other.txt && git commit -q -m "feat: other"
  run_hook "Bash" "$payload"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
}

@test "push-review: a clean review still passes under the default preset" {
  gate_config '{"version":1}'
  write_state "$(current_diff_sha)" 0
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "push-review: open findings advise rather than deny under the default preset" {
  gate_config '{"version":1}'
  write_state "$(current_diff_sha)" 2
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" == *"open findings"* ]]
  [ ! -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "push-review: per-gate off emits nothing at all" {
  gate_config '{"version":1,"gates":{"pushReview":{"mode":"off"}}}'
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "push-review: advise mode reports without deciding" {
  gate_config '{"version":1,"gates":{"pushReview":{"mode":"advise"}}}'
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" == *"Code review required"* ]]
  [ ! -f "$STATE_DIR/feat_example.pending-waiver.json" ]
}

@test "push-review: ask degrades to advise on codex" {
  use_ask_mode
  payload=$(build_input "git push")
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=development \
    TOOLU_HOST_OVERRIDE=codex run bash "$HOOK_SCRIPT" <<<"$payload"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [ -n "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" ]
}

@test "push-review: an empty diff advises under the default preset" {
  gate_config '{"version":1}'
  git checkout -q -b feat/empty development
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$reason" == *"is empty"* ]]
}

@test "push-review: telemetry records the waived push" {
  use_ask_mode
  export TELEMETRY_DIR="$SANDBOX/.claude/tmp/telemetry"
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  promote_waiver "$(current_diff_sha)"
  run_hook "Bash" "$payload"
  grep -q '"reason_code":"waived"' "$TELEMETRY_DIR/feat_example.jsonl"
  grep -q '"result":"ask"' "$TELEMETRY_DIR/feat_example.jsonl"
}
