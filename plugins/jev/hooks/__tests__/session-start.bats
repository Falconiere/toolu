#!/usr/bin/env bats
# session-start.sh publishes the jev wrapper at a stable path the agent's Bash
# tool can reach without $CLAUDE_PLUGIN_ROOT, on either host.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/jev/scripts" && pwd)/jev.sh"
}

teardown() {
  unset TOOLU_HOST_OVERRIDE CODEX_HOME TOOLU_CONFIG_DIR
  rm -rf "$TMP"
}

@test "session-start: publishes wrapper symlink at \$CLAUDE_CONFIG_DIR/jev/jev.sh" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/jev/jev.sh"
  [ -L "$dst" ]
  [ "$(readlink "$dst")" = "$SRC" ]
  [ -x "$dst" ]
}

@test "session-start: publishes under \$CODEX_HOME when the host is codex" {
  export TOOLU_HOST_OVERRIDE=codex
  export CODEX_HOME="$TMP/codex"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$CODEX_HOME/jev/jev.sh")" = "$SRC" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}

@test "session-start: refreshes a stale symlink to the current target" {
  pub_root="$CLAUDE_CONFIG_DIR/jev"
  mkdir -p "$pub_root"
  ln -s /nonexistent/old/jev.sh "$pub_root/jev.sh"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$pub_root/jev.sh")" = "$SRC" ]
}

@test "session-start: NEVER clobbers a real file at the wrapper path" {
  pub_root="$CLAUDE_CONFIG_DIR/jev"
  mkdir -p "$pub_root"
  printf '#!/usr/bin/env bash\necho user-override\n' > "$pub_root/jev.sh"
  chmod +x "$pub_root/jev.sh"
  before=$(cat "$pub_root/jev.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -L "$pub_root/jev.sh" ]
  [ "$(cat "$pub_root/jev.sh")" = "$before" ]
}

@test "session-start: emits nothing on stdout (SessionStart hygiene)" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "session-start: idempotent (second run leaves the symlink identical)" {
  bash "$HOOK" <<<'{}'
  before=$(readlink "$CLAUDE_CONFIG_DIR/jev/jev.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(readlink "$CLAUDE_CONFIG_DIR/jev/jev.sh")
  [ "$before" = "$after" ]
}

# Fail-soft: a corrupted install where skills/ is missing must NOT break the
# session.
@test "session-start: source wrapper missing -> exits 0, no symlink, silent (fail-soft)" {
  fake="$TMP/fake-plugin/hooks"
  mkdir -p "$fake"
  cp "$HOOK" "$fake/session-start.sh"
  run bash "$fake/session-start.sh" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}
