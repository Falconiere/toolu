#!/usr/bin/env bats
# Real-data tests for write-state.sh. No mocks — real temp git repos. The core
# guarantee: the script's diff_sha/base/slug/reviewed_files match the toolu
# push-review gate's recipe, so a state file it writes is accepted (not
# rejected as stale or as incomplete file coverage).

WS="${BATS_TEST_DIRNAME}/../write-state.sh"
# The real gate this writer's output must satisfy (schema v2, reviewed_files
# contract) — cross-plugin path, not sourced, only invoked as a subprocess.
GATE="${BATS_TEST_DIRNAME}/../../../../../toolu/hooks/pre-tools/modules/push-review.sh"

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
  # Resolved path: on macOS mktemp -d hands back /var/... while git reports the
  # /private/var/... realpath, and the writer keys off the git root.
  TMP_REAL=$(pwd -P)
  git init -q -b main
  git config user.email t@t
  git config user.name t
  git config commit.gpgsign false
  git commit --allow-empty -qm init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "write-state: diff_sha matches gate recipe for a non-main base (origin/HEAD)" {
  git checkout -q -b develop
  git commit --allow-empty -qm devbase
  git update-ref refs/remotes/origin/develop refs/heads/develop
  git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/develop
  git checkout -q -b feature
  echo x > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0)
  [ "$(jq -r .base_branch "$out")" = "develop" ]
  [ "$(jq -r .diff_sha "$out")" = "$(git diff --no-color develop...HEAD | git hash-object --stdin)" ]
}

@test "write-state: diff_sha matches gate recipe with main fallback (no origin/HEAD)" {
  git checkout -q -b feature
  echo y > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0)
  [ "$(jq -r .base_branch "$out")" = "main" ]
  [ "$(jq -r .diff_sha "$out")" = "$(git diff --no-color main...HEAD | git hash-object --stdin)" ]
}

@test "write-state: \$PUSH_REVIEW_BASE override is honored (matches gate)" {
  git checkout -q -b feature
  echo z > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0)
  [ "$(jq -r .base_branch "$out")" = "main" ]
  [ "$(jq -r .diff_sha "$out")" = "$(git diff --no-color main...HEAD | git hash-object --stdin)" ]
}

@test "write-state: slug maps feat/x-y -> feat_x-y" {
  git checkout -q -b feat/x-y
  echo z > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0)
  [ "$out" = "$TMP_REAL/.claude/tmp/push-review/feat_x-y.json" ]
}

@test "write-state: Codex host stores attestations under .codex/tmp" {
  git checkout -q -b feature
  echo z > f.txt && git add f.txt && git commit -qm work
  out=$(TOOLU_HOST_OVERRIDE=codex bash "$WS" --findings-count 0)
  [ "$out" = "$TMP_REAL/.codex/tmp/push-review/feature.json" ]
  [ ! -e "$TMP_REAL/.claude/tmp/push-review/feature.json" ]
}

@test "write-state: writes schema and bumps review_round 0->1->2 on an unchanged diff" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0 --reviewers '["toolu-review:review"]')
  [ "$(jq -r .version "$out")" = "2" ]
  [ "$(jq -r .findings_count "$out")" = "0" ]
  [ "$(jq -r '.reviewers[0]' "$out")" = "toolu-review:review" ]
  [ "$(jq -r .review_round "$out")" = "1" ]
  out2=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0)
  [ "$(jq -r .review_round "$out2")" = "2" ]
}

@test "write-state: review_round restarts at 1 when the diff changes" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  out=$(bash "$WS" --findings-count 0)
  out=$(bash "$WS" --findings-count 0)
  [ "$(jq -r .review_round "$out")" = "2" ]

  # New commit → new diff_sha → the prior rounds judged code that no longer
  # exists, so the counter restarts instead of marching toward MAX_ROUNDS.
  echo b >> f.txt && git add f.txt && git commit -qm more
  out=$(bash "$WS" --findings-count 0)
  [ "$(jq -r .review_round "$out")" = "1" ]
  [ "$(jq -r .diff_sha "$out")" = "$(git diff --no-color main...HEAD | git hash-object --stdin)" ]
}

@test "write-state: state file lands under the repo root, not \$CLAUDE_PROJECT_DIR" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  elsewhere=$(mktemp -d)
  out=$(CLAUDE_PROJECT_DIR="$elsewhere" bash "$WS" --findings-count 0)
  [ "$out" = "$TMP_REAL/.claude/tmp/push-review/feature.json" ]
  [ ! -e "$elsewhere/.claude/tmp/push-review/feature.json" ]
  rm -rf "$elsewhere"
}

@test "write-state: --repo targets a worktree's own state dir" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  git checkout -q main
  wt="$TMP/wt"
  git worktree add -q "$wt" feature
  wt_real=$(cd "$wt" && pwd -P)

  # Run from the main checkout, naming the worktree — the gate reads the state
  # file under the pushed repo's root, so the writer must land it there.
  out=$(bash "$WS" --findings-count 0 --repo "$wt")
  [ "$out" = "$wt_real/.claude/tmp/push-review/feature.json" ]
  [ "$(jq -r .branch "$out")" = "feature" ]
  [ "$(jq -r .diff_sha "$out")" = "$(git -C "$wt" diff --no-color main...HEAD | git hash-object --stdin)" ]
  [ ! -e "$TMP_REAL/.claude/tmp/push-review/feature.json" ]
}

@test "write-state: \$STATE_DIR override is honored (mirrors the gate)" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  out=$(STATE_DIR="$TMP/custom" bash "$WS" --findings-count 0)
  [ "$out" = "$TMP/custom/feature.json" ]
}

@test "write-state: --repo outside a git repo fails loudly" {
  notrepo=$(mktemp -d)
  run bash "$WS" --findings-count 0 --repo "$notrepo"
  [ "$status" -eq 1 ]
  [[ "$output" == *"not inside a git repo"* ]]
  rm -rf "$notrepo"
}

@test "write-state: refuses the empty-blob SHA (no divergence from base)" {
  git checkout -q -b feature   # branched from main, no new commit → empty diff
  run env CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0
  [ "$status" -eq 1 ]
  [[ "$output" == *"empty"* ]]
  [ ! -f "$TMP/.claude/tmp/push-review/feature.json" ]
}

@test "write-state: non-integer findings-count is rejected" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  run env CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count notanumber
  [ "$status" -eq 2 ]
}

@test "write-state: reviewed_files is auto-computed from the real diff (v2)" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  mkdir -p sub && echo b > sub/g.txt && git add sub/g.txt && git commit -qm work2
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0)
  expected=$(git diff --no-color main...HEAD --name-only | sort -u \
    | jq -R -s -c 'split("\n") | map(select(length > 0))')
  [ "$(jq -c .reviewed_files "$out")" = "$expected" ]
  [[ "$(jq -c .reviewed_files "$out")" == *"f.txt"* ]]
  [[ "$(jq -c .reviewed_files "$out")" == *"sub/g.txt"* ]]
}

@test "write-state: --reviewed-files overrides the auto-computed list" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  echo b > g.txt && git add g.txt && git commit -qm work2
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0 --reviewed-files "f.txt")
  # The real diff touched both f.txt and g.txt, but the override names only
  # f.txt — proving the override replaces auto-detection instead of merging
  # with it (a merge would also list g.txt here).
  [ "$(jq -c .reviewed_files "$out")" = '["f.txt"]' ]
}

@test "write-state: --reviewed-files dedupes and sorts, dropping empty entries" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0 \
    --reviewed-files "b.txt,a.txt,,a.txt")
  [ "$(jq -c .reviewed_files "$out")" = '["a.txt","b.txt"]' ]
}

@test "write-state: emitted v2 state satisfies the real push-review gate (allow)" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  echo b > g.txt && git add g.txt && git commit -qm work2
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0 \
    --reviewers '["toolu-review:review"]')
  [ -f "$out" ]
  [ "$(jq -r .version "$out")" = "2" ]

  # Drive the REAL gate against the state this writer just produced — the
  # writer<->gate contract this test exists to prove. Same cwd (the sandbox
  # repo) and same base override the writer used, so both resolve identical
  # repo_root/diff_sha/reviewed_files.
  payload=$(jq -n '{tool_name:"Bash", tool_input:{command:"git push origin feature"}}')
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=main \
    run bash "$GATE" <<<"$payload"
  [ "$status" -eq 0 ]
  decision=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // empty')
  [ "$decision" != "deny" ]
}

@test "write-state: a state missing a changed file still denies at the real gate" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  echo b > g.txt && git add g.txt && git commit -qm work2
  out=$(CLAUDE_PROJECT_DIR="$TMP" PUSH_REVIEW_BASE=main bash "$WS" --findings-count 0 \
    --reviewed-files "f.txt")
  [ -f "$out" ]

  payload=$(jq -n '{tool_name:"Bash", tool_input:{command:"git push origin feature"}}')
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=main \
    run bash "$GATE" <<<"$payload"
  [ "$status" -eq 0 ]
  decision=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // empty')
  [ "$decision" = "deny" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" == *"g.txt"* ]]
}
