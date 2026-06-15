#!/usr/bin/env bats
# Tests for the shared repo-scope lib. Real git repos + real linked worktrees —
# no mocks. Asserts the canonical key is the main-repo basename and stays STABLE
# across worktrees (the invariant that keeps memory scope shared), and that the
# function degrades to empty outside any repo.

LIB="${BATS_TEST_DIRNAME}/../repo-scope.sh"

setup() {
  # shellcheck source=../repo-scope.sh
  . "$LIB"
  TMP=$(mktemp -d)
}

teardown() {
  cd /tmp || return
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "repo-scope: plain checkout key is the repo dir basename" {
  repo="$TMP/myproj"
  mkdir -p "$repo" && ( cd "$repo" && git init -q )
  [ "$(comemory_repo_key "$repo")" = "myproj" ]
}

@test "repo-scope: defaults to cwd when no dir arg" {
  repo="$TMP/proj2"
  mkdir -p "$repo" && ( cd "$repo" && git init -q )
  ( cd "$repo" && [ "$(comemory_repo_key)" = "proj2" ] )
}

@test "repo-scope: a linked worktree resolves to the MAIN repo basename" {
  repo="$TMP/mainrepo"
  mkdir -p "$repo"
  ( cd "$repo" && git init -q && git -c user.email=a@b.c -c user.name=t commit -q --allow-empty -m init )
  wt="$TMP/wt-feature"
  ( cd "$repo" && git worktree add -q "$wt" -b feature )
  # The worktree dir basename is "wt-feature", but the scope must be "mainrepo".
  [ "$(comemory_repo_key "$wt")" = "mainrepo" ]
}

@test "repo-scope: custom GIT_DIR (.git not <root>/.git) uses the --show-toplevel fallback" {
  # The exact case the old status-hook repo_key got wrong: git-common-dir is not
  # "<root>/.git", so the */.git branch misses and we must fall back to
  # --show-toplevel's basename. (This fallback existed in detect_scope/
  # detect_project_root but NOT the original repo_key.)
  work="$TMP/work"
  gitdir="$TMP/elsewhere.git"
  git init -q --separate-git-dir="$gitdir" "$work"
  [ "$(comemory_repo_key "$work")" = "work" ]
}

@test "repo-scope: empty (rc 0) outside any git repo" {
  notrepo="$TMP/plain"
  mkdir -p "$notrepo"
  run comemory_repo_key "$notrepo"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "version_ge: orders versions correctly" {
  version_ge "0.9.0" "0.8.0"
  version_ge "0.8.0" "0.8.0"
  run version_ge "0.7.0" "0.8.0"
  [ "$status" -ne 0 ]
}
