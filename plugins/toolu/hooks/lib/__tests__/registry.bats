#!/usr/bin/env bats
# Tests for hooks/lib/registry.sh

setup() {
  . "${BATS_TEST_DIRNAME}/../registry.sh"
  TMP=$(mktemp -d)
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR
}
teardown() { rm -rf "$TMP"; }

@test "registry_event_dir honors CLAUDE_CONFIG_DIR" {
  run env CLAUDE_CONFIG_DIR="$TMP/cfg" bash -c '
    . "'"${BATS_TEST_DIRNAME}"'/../registry.sh"
    toolu_registry_event_dir PreToolUse
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/cfg/toolu/pre-tools.d" ]
}

@test "registry_event_dir honors TOOLU_CONFIG_DIR over CLAUDE_CONFIG_DIR" {
  run env TOOLU_CONFIG_DIR="$TMP/cfg" CLAUDE_CONFIG_DIR="$TMP/wrong" bash -c '
    . "'"${BATS_TEST_DIRNAME}"'/../registry.sh"
    toolu_registry_event_dir PreToolUse
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/cfg/toolu/pre-tools.d" ]
}

@test "registry_event_dir falls back to HOME/.claude" {
  run env -u CLAUDE_CONFIG_DIR -u TOOLU_CONFIG_DIR HOME="$TMP/home" bash -c '
    . "'"${BATS_TEST_DIRNAME}"'/../registry.sh"
    toolu_registry_event_dir PostToolUse
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/home/.claude/toolu/post-tools.d" ]
}

@test "registry_event_dir uses CODEX_HOME for Codex" {
  run env -u CLAUDE_CONFIG_DIR -u TOOLU_CONFIG_DIR TOOLU_HOST_OVERRIDE=codex \
    CODEX_HOME="$TMP/codex" HOME="$TMP/home" bash -c '
    . "'"${BATS_TEST_DIRNAME}"'/../registry.sh"
    toolu_registry_event_dir PreToolUse
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/codex/toolu/pre-tools.d" ]
}

@test "registry_event_dir maps unknown event to a sanitized name" {
  run bash -c '
    . "'"${BATS_TEST_DIRNAME}"'/../registry.sh"
    CLAUDE_CONFIG_DIR="'"$TMP"'" toolu_registry_event_dir SessionStart
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/toolu/session-start.d" ]
}

@test "Codex registry pruning removes only modules absent from a ready plugin snapshot" {
  root="$TMP/codex/toolu"
  mkdir -p "$root/pre-tools.d" "$root/post-tools.d"
  printf '#!/bin/sh\n' > "$root/pre-tools.d/comemory@toolu__stale.sh"
  printf '#!/bin/sh\n' > "$root/pre-tools.d/ts-quality@toolu__active.sh"
  printf '#!/bin/sh\n' > "$root/pre-tools.d/unnamespaced.sh"
  printf '%s\n' '{"version":1,"status":"ready","plugins":["ts-quality@toolu"]}' > "$TMP/plugins.json"

  TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex" TOOLU_CODEX_PLUGIN_SNAPSHOT="$TMP/plugins.json" \
    run toolu_registry_prune_inactive
  [ "$status" -eq 0 ]
  [ ! -e "$root/pre-tools.d/comemory@toolu__stale.sh" ]
  [ -f "$root/pre-tools.d/ts-quality@toolu__active.sh" ]
  [ -f "$root/pre-tools.d/unnamespaced.sh" ]
}

@test "Codex registry pruning is fail-open for an indeterminate snapshot" {
  root="$TMP/codex/toolu/pre-tools.d"
  mkdir -p "$root"
  printf '#!/bin/sh\n' > "$root/comemory@toolu__keep.sh"
  printf '%s\n' '{"version":1,"status":"indeterminate","plugins":[]}' > "$TMP/plugins.json"
  TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex" TOOLU_CODEX_PLUGIN_SNAPSHOT="$TMP/plugins.json" \
    run toolu_registry_prune_inactive
  [ "$status" -eq 0 ]
  [ -f "$root/comemory@toolu__keep.sh" ]
}
