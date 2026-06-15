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
