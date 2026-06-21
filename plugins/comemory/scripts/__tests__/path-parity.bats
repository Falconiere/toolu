#!/usr/bin/env bats
# Path-parity guard (spec AC-11 / plan S9): the setup_done marker WRITER in
# scripts/setup.sh must target the SAME file the READER
# plugins/toolu/hooks/lib/config.sh:_toolu_project_cfg resolves — across the
# worktree case and every env override. A mismatch means the opt-in marker lands
# where session-start can't read it: silent breakage.
#
# Method (real data, no mocks): drive setup.sh to READY via the version-stub seam
# so the writer actually runs, then compute the EXPECTED path by SOURCING config.sh
# and calling _toolu_project_cfg under the SAME env. Assert the marker file exists
# at that resolver-computed path with .comemory.setup_done == true (jq). This
# compares the writer's real target to the reader's real resolver — not a
# hard-coded path. (The no-sourcing rule is a runtime constraint on setup.sh, not
# on this test; the test MAY source config.sh to get the canonical expectation.)

SETUP_SH="${BATS_TEST_DIRNAME}/../setup.sh"
CONFIG_SH="${BATS_TEST_DIRNAME}/../../../toolu/hooks/lib/config.sh"

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  # Isolate the comemory store so the READY path never touches the real one.
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/data"
  mkdir -p "$COMEMORY_DATA_DIR"
}

teardown() {
  # Drop any env overrides a scenario set, so they never leak between tests.
  unset TOOLU_PROJECT_DIR CLAUDE_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME
}

# Canonicalize a (possibly symlinked) dir to its physical path. On macOS
# $BATS_TEST_TMPDIR lives under /var -> /private/var, and `git rev-parse
# --show-toplevel` (used by BOTH writer and reader) returns the /private form.
# We canonicalize the test's own path literals the same way so they line up with
# the resolver — this is a fixture-symlink artifact, not behavior under test.
_canon() { ( cd "$1" 2>/dev/null && pwd -P ); }

# A fresh git repo with one indexable file + commit, so the READY path's
# install-hooks/index-code have something real to act on. Sets REPO (canonical).
_make_repo() {
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"
  git init -q "$REPO"
  git -C "$REPO" config user.email t@t.t
  git -C "$REPO" config user.name t
  printf 'fn main() {}\n' > "$REPO/main.rs"
  git -C "$REPO" add -A
  git -C "$REPO" commit -q -m init
  REPO=$(_canon "$REPO")
}

# A fake `comemory` whose --version prints a value above the floor, so the READY
# branch (and thus the marker writer) runs without a real binary. Echoes the
# stub path. Mirrors setup.bats's _version_stub seam.
_version_stub() {
  local ver="$1" dir="$BATS_TEST_TMPDIR/vstub-$BATS_TEST_NUMBER"
  mkdir -p "$dir"
  printf '#!/bin/sh\ncase "$1" in --version) echo "%s";; *) exit 0;; esac\n' "$ver" > "$dir/comemory"
  chmod +x "$dir/comemory"
  printf '%s\n' "$dir/comemory"
}

# Compute the READER's canonical config path by sourcing config.sh and calling
# _toolu_project_cfg under the CURRENT env + cwd — the same inputs the writer saw.
# Runs in a subshell so the sourced functions never pollute the bats process.
_reader_path() {
  ( . "$CONFIG_SH" && _toolu_project_cfg )
}

# Drive setup.sh to READY (env passed through), then assert the file the READER
# resolves exists and carries setup_done==true. $1.. = extra `env` assignments.
_assert_parity() {
  local stub
  stub=$(_version_stub "comemory 9.9.9")
  run env COMEMORY="$stub" "$@" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
  [[ "$output" == *"memory: enabled"* ]]

  # The reader's resolver, under the same env the writer ran with.
  local expected
  expected=$(env "$@" bash -c '. "$1" && _toolu_project_cfg' _ "$CONFIG_SH")
  [ -n "$expected" ]
  if [ ! -f "$expected" ]; then
    # Surface writer-vs-reader divergence for the worktree regression.
    local actual
    actual=$(printf '%s\n' "$output" | sed -n 's/.*setup_done marker written: \(.*\))/\1/p')
    echo "READER expected path: $expected" >&2
    echo "WRITER actual path:   ${actual:-<none reported>}" >&2
    false
  fi
  # Existence at $expected is necessary but NOT sufficient: under symlinks or a
  # second config dir the file could exist there while the writer actually
  # targeted a different literal path. Assert the writer's reported target string
  # EQUALS the reader's resolver — the parity this test exists to guard.
  local writer_path
  writer_path=$(printf '%s\n' "$output" | sed -n 's/.*setup_done marker written: \(.*\))/\1/p')
  [ "$writer_path" = "$expected" ]   # writer target must equal reader resolver
  [ "$(jq -r '.comemory.setup_done' "$expected")" = "true" ]
}

@test "parity: plain repo (no overrides) -> <repo>/.claude/toolu.config.json" {
  _make_repo
  cd "$REPO"
  _assert_parity
  # And it is exactly the .claude path under the repo root.
  [ -f "$REPO/.claude/toolu.config.json" ]
}

@test "parity: real git worktree, run INSIDE the worktree, resolves to the WORKTREE not the main repo" {
  _make_repo
  local wt="$BATS_TEST_TMPDIR/wt"
  git -C "$REPO" worktree add -q "$wt" -b wt-branch
  wt=$(_canon "$wt")
  cd "$wt"

  # Sanity: this really is a worktree (.git is a file, not a dir) and
  # --show-toplevel points at the worktree, while --git-common-dir points back
  # at the main repo — the exact trap the writer must NOT fall into.
  [ -f "$wt/.git" ]
  [ "$(git -C "$wt" rev-parse --show-toplevel)" = "$wt" ]

  _assert_parity

  # Hard regression guard: the marker is under the WORKTREE, never the main repo.
  [ -f "$wt/.claude/toolu.config.json" ]
  [ ! -f "$REPO/.claude/toolu.config.json" ]
  # And the reader agrees the target is the worktree path.
  cd "$wt"
  [ "$(_reader_path)" = "$wt/.claude/toolu.config.json" ]
}

@test "parity: TOOLU_PROJECT_DIR override -> \$TOOLU_PROJECT_DIR/.claude/toolu.config.json" {
  _make_repo
  local proj="$BATS_TEST_TMPDIR/proj-toolu"
  mkdir -p "$proj"; proj=$(_canon "$proj")
  # cwd is the repo, but the override must win over --show-toplevel.
  cd "$REPO"
  _assert_parity TOOLU_PROJECT_DIR="$proj"
  [ -f "$proj/.claude/toolu.config.json" ]
}

@test "parity: CLAUDE_PROJECT_DIR override (TOOLU_PROJECT_DIR unset) -> \$CLAUDE_PROJECT_DIR/.claude/toolu.config.json" {
  _make_repo
  local proj="$BATS_TEST_TMPDIR/proj-claude"
  mkdir -p "$proj"; proj=$(_canon "$proj")
  cd "$REPO"
  _assert_parity CLAUDE_PROJECT_DIR="$proj"
  [ -f "$proj/.claude/toolu.config.json" ]
}

@test "parity: TOOLU_PROJECT_CONFIG_DIRNAME override -> <root>/.cfg/toolu.config.json" {
  _make_repo
  cd "$REPO"
  _assert_parity TOOLU_PROJECT_CONFIG_DIRNAME=".cfg"
  [ -f "$REPO/.cfg/toolu.config.json" ]
}
