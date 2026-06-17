#!/usr/bin/env bats
# Real-data tests for write-state.sh. No mocks — real temp git repos. The core
# guarantee: the script's diff_sha/base/slug match the toolu push-review
# gate's recipe, so a state file it writes is accepted (not rejected as stale).

WS="${BATS_TEST_DIRNAME}/../write-state.sh"

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
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
  [ "$out" = "$TMP/.claude/tmp/push-review/feat_x-y.json" ]
}

@test "write-state: writes schema and bumps review_round 0->1->2" {
  git checkout -q -b feature
  echo a > f.txt && git add f.txt && git commit -qm work
  out=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0 --reviewers '["code-review:review"]')
  [ "$(jq -r .version "$out")" = "2" ]
  [ "$(jq -r .findings_count "$out")" = "0" ]
  [ "$(jq -r '.reviewers[0]' "$out")" = "code-review:review" ]
  [ "$(jq -r .review_round "$out")" = "1" ]
  out2=$(CLAUDE_PROJECT_DIR="$TMP" bash "$WS" --findings-count 0)
  [ "$(jq -r .review_round "$out2")" = "2" ]
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
