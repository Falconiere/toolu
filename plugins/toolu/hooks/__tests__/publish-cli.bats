#!/usr/bin/env bats
# Tests for hooks/publish-cli.sh — publishes the toolu-claude launcher symlink to
# a stable, version-independent path under CLAUDE_CONFIG_DIR. Real filesystem, no
# stubs; symlink targets are asserted by full identity.

HOOK="${BATS_TEST_DIRNAME}/../publish-cli.sh"

setup() {
  TMP=$(mktemp -d)
  # Fake plugin layout: <plugin>/hooks/publish-cli.sh + <plugin>/bin/toolu-claude.
  mkdir -p "$TMP/plugin/hooks" "$TMP/plugin/bin"
  cp "$HOOK" "$TMP/plugin/hooks/publish-cli.sh"
  chmod +x "$TMP/plugin/hooks/publish-cli.sh"
  : > "$TMP/plugin/bin/toolu-claude"
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
  unset CLAUDE_CONFIG_DIR
}

@test "publishes launcher symlink to CLAUDE_CONFIG_DIR/toolu/bin/toolu-claude" {
  run bash "$TMP/plugin/hooks/publish-cli.sh" </dev/null
  [ "$status" -eq 0 ]
  dst="$TMP/cfg/toolu/bin/toolu-claude"
  [ -L "$dst" ]
  [ "$(readlink "$dst")" = "$TMP/plugin/bin/toolu-claude" ]
}

@test "idempotent: re-running keeps the symlink pointing at the launcher" {
  bash "$TMP/plugin/hooks/publish-cli.sh" </dev/null
  run bash "$TMP/plugin/hooks/publish-cli.sh" </dev/null
  [ "$status" -eq 0 ]
  [ "$(readlink "$TMP/cfg/toolu/bin/toolu-claude")" = "$TMP/plugin/bin/toolu-claude" ]
}

@test "never clobbers a real file the user placed at the destination" {
  mkdir -p "$TMP/cfg/toolu/bin"
  printf 'user-owned\n' > "$TMP/cfg/toolu/bin/toolu-claude"
  run bash "$TMP/plugin/hooks/publish-cli.sh" </dev/null
  [ "$status" -eq 0 ]
  [ ! -L "$TMP/cfg/toolu/bin/toolu-claude" ]
  grep -qF "user-owned" "$TMP/cfg/toolu/bin/toolu-claude"
}

@test "no-op (exit 0, no symlink) when the launcher is absent from the plugin" {
  rm "$TMP/plugin/bin/toolu-claude"
  run bash "$TMP/plugin/hooks/publish-cli.sh" </dev/null
  [ "$status" -eq 0 ]
  [ ! -e "$TMP/cfg/toolu/bin/toolu-claude" ]
}
