#!/usr/bin/env bats
# register.sh — registry sync + gb shim install. AC#13.

SCRIPT="${BATS_TEST_DIRNAME}/../register.sh"

setup() {
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  unset TOOLU_CONFIG_DIR
  REG="$CLAUDE_CONFIG_DIR/toolu"
  echo "" | bash "$SCRIPT"   # register consumes stdin, silent on success
}

@test "pre-tools nudges synced under git-better@toolu prefix" {
  [ -f "$REG/pre-tools.d/git-better@toolu__git-lean-nudge.sh" ]
  [ -f "$REG/pre-tools.d/git-better@toolu__git-conventions-nudge.sh" ]
}

@test "post-tools byte-savings synced" {
  [ -f "$REG/post-tools.d/git-better@toolu__git-byte-savings.sh" ]
}

@test "gb shim installed, executable, references the wrapper" {
  shim="$REG/bin/gb"
  [ -f "$shim" ]
  [ -x "$shim" ]
  grep -q 'git-better.sh' "$shim"
  grep -q 'TOOLU_HOST_OVERRIDE="claude"' "$shim"
}

@test "Codex gb shim preserves host identity without CODEX_HOME at invocation" {
  codex_home="$BATS_TEST_TMPDIR/codex-home"
  run env -u CLAUDE_CONFIG_DIR PLUGIN_ROOT="$BATS_TEST_TMPDIR/plugin" \
    CODEX_HOME="$codex_home" bash -c 'printf "\n" | bash "$1"' _ "$SCRIPT"
  [ "$status" -eq 0 ]
  shim="$codex_home/toolu/bin/gb"
  [ -x "$shim" ]
  grep -q 'TOOLU_HOST_OVERRIDE="codex"' "$shim"

  REPO="$BATS_TEST_TMPDIR/codex-repo"
  mkdir -p "$REPO" "$BATS_TEST_TMPDIR/home"
  git -C "$REPO" init -q
  git -C "$REPO" config user.email t@e.com
  git -C "$REPO" config user.name t
  printf 'x\n' > "$REPO/f"
  git -C "$REPO" add f
  git -C "$REPO" commit -qm init
  run env -u CODEX_HOME -u CLAUDE_CONFIG_DIR HOME="$BATS_TEST_TMPDIR/home" \
    bash -c 'cd "$1"; bash "$2" conventions --json' _ "$REPO" "$shim"
  [ "$status" -eq 0 ]
  [ -d "$BATS_TEST_TMPDIR/home/.codex/toolu/git-better/conventions" ]
  [ ! -d "$BATS_TEST_TMPDIR/home/.claude/toolu/git-better/conventions" ]
}

@test "gb shim actually execs the wrapper" {
  shim="$REG/bin/gb"
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q; git config user.email t@e.com; git config user.name t
  printf 'a\n' > f; git add -A; git commit -qm "init"
  run bash "$shim" log -n 1
  [ "$status" -eq 0 ]
  [ -n "$output" ]
}

@test "prune: removes our orphaned registry entries on next run" {
  orphan="$REG/pre-tools.d/git-better@toolu__gone.sh"
  printf '#!/bin/bash\n' > "$orphan"
  echo "" | bash "$SCRIPT"
  [ ! -f "$orphan" ]
}

@test "silent on success (no stdout)" {
  run bash -c "echo '' | bash '$SCRIPT'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
