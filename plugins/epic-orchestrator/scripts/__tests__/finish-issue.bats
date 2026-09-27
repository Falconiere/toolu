#!/usr/bin/env bats
# finish_issue.sh snapshots the worktree to refs/epic-wip/<key> before teardown.

SCRIPTS="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"

setup() {
  export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
  TMP="$(mktemp -d)"
  git init -q --bare -b main "$TMP/origin.git"
  git clone -q "$TMP/origin.git" "$TMP/main" 2>/dev/null
  echo one >"$TMP/main/a.txt"
  git -C "$TMP/main" add a.txt
  git -C "$TMP/main" commit -qm init
  git -C "$TMP/main" push -q origin main
  git -C "$TMP/main" worktree add -q -b feat/1-x "$TMP/wt" origin/main
  mkdir -p "$TMP/state/issues"
  # No agent and no workspace_id: the teardown never reaches herdr.
  jq -n --arg wt "$TMP/wt" --arg co "$TMP/main" \
    '{key:"k-1", repo:"o/r", branch:"feat/1-x", checkout:$co, worktree:$wt, stage:"running"}' \
    >"$TMP/state/issues/k-1.json"
}

teardown() { rm -rf "$TMP"; }

@test "abandon snapshots uncommitted and untracked work and records the ref" {
  echo two >"$TMP/wt/a.txt"
  echo new >"$TMP/wt/new.txt"
  run bash "$SCRIPTS/finish_issue.sh" "$TMP/state" k-1 --abandon
  [ "$status" -eq 0 ]
  [ "$(jq -r .stage "$TMP/state/issues/k-1.json")" = abandoned ]
  [ "$(jq -r .wip_ref "$TMP/state/issues/k-1.json")" = refs/epic-wip/k-1 ]
  [ "$(git -C "$TMP/main" show refs/epic-wip/k-1:a.txt)" = two ]
  [ "$(git -C "$TMP/main" show refs/epic-wip/k-1:new.txt)" = new ]
}

@test "clean, pushed worktree records no wip ref" {
  run bash "$SCRIPTS/finish_issue.sh" "$TMP/state" k-1 --abandon
  [ "$status" -eq 0 ]
  [ "$(jq -r .wip_ref "$TMP/state/issues/k-1.json")" = null ]
  run git -C "$TMP/main" rev-parse --verify --quiet refs/epic-wip/k-1
  [ "$status" -ne 0 ]
}
