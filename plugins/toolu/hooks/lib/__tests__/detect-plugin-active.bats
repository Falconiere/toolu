#!/usr/bin/env bats
# Tests for toolu_plugin_active in hooks/lib/detect.sh
#
# Contract under test (three behaviors):
#   - plugin present in installed manifest        -> return 0
#   - plugin definitively absent                  -> return 1
#   - indeterminate (no manifest / unreadable)    -> return 0 (fail-open)
#
# NOTE on the argument: detect_plugin_installed (and therefore
# toolu_plugin_active) is queried by the FULL "name@marketplace" spec via
# `.plugins | has($spec)` — NOT by the bare plugin name. So the spec we pass and
# the JSON key we write must be identical. We exercise the real resolution path:
# detect_plugin_installed reads $CLAUDE_PLUGINS_REGISTRY, falling back to
# $HOME/.claude/plugins/installed_plugins.json; here we drive it via $HOME.

setup() {
  . "${BATS_TEST_DIRNAME}/../detect.sh"
  TMP=$(mktemp -d)
  export HOME="$TMP"
  # Ensure the fixture under $HOME is the file detect_plugin_installed reads
  # (clear every inherited override in the resolution chain — CLAUDE_CONFIG_DIR
  # et al. would otherwise point at a real ~/.claude-work manifest).
  unset CLAUDE_PLUGINS_REGISTRY TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR PI_CODING_AGENT_DIR
  mkdir -p "$TMP/.claude/plugins"
}
teardown() { rm -rf "$TMP"; }

_write_installed() {
  printf '%s' "$1" > "$TMP/.claude/plugins/installed_plugins.json"
}

@test "active: returns 0 when plugin present in installed_plugins.json" {
  _write_installed '{"plugins":{"comemory@toolu":{}}}'
  run toolu_plugin_active comemory@toolu
  [ "$status" -eq 0 ]
}

@test "active: returns 1 when plugin absent" {
  _write_installed '{"plugins":{"other@toolu":{}}}'
  run toolu_plugin_active comemory@toolu
  [ "$status" -eq 1 ]
}

@test "active: indeterminate (no manifest) defaults to active (fail-open)" {
  rm -f "$TMP/.claude/plugins/installed_plugins.json"
  run toolu_plugin_active comemory@toolu
  [ "$status" -eq 0 ]
}
