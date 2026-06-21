#!/usr/bin/env bats
# The SessionStart dependency-warning block: given a plugin manifest declaring
# dependencies, it must WARN (with an install command) for each dep absent from
# the installed-plugins registry, stay silent when all are present, and suppress
# all warnings when the registry is indeterminate (jq/registry unavailable).
#
# Drives the REAL entrypoint (session-start.sh) with CLAUDE_PLUGIN_ROOT pointed
# at a synthetic plugin dir and CLAUDE_PLUGINS_REGISTRY at a synthetic manifest.

setup() {
  PLUGINS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  ENTRY="$PLUGINS_DIR/toolu/hooks/session-start.sh"
  TMP=$(mktemp -d)
  PLUGROOT="$TMP/plug"; mkdir -p "$PLUGROOT/.claude-plugin"
  REG="$TMP/installed_plugins.json"
}
teardown() { [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"; }

# Write the plugin manifest under test (full JSON on stdin).
_manifest() {
  cat > "$PLUGROOT/.claude-plugin/plugin.json"
}

_run_entry() {
  env CLAUDE_PLUGIN_ROOT="$PLUGROOT" \
    CLAUDE_PLUGINS_REGISTRY="$REG" \
    HOME="$TMP" \
    bash "$ENTRY" <<<'{"hook_event_name":"SessionStart","source":"startup"}'
}

@test "dep-warning: warns with install command for a missing dependency" {
  _manifest <<'JSON'
{"name":"toolu","dependencies":[{"name":"caveman","marketplace":"caveman"}]}
JSON
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "required plugins missing"
  echo "$output" | grep -q "/plugin install caveman@caveman"
}

@test "dep-warning: silent when every dependency is installed" {
  _manifest <<'JSON'
{"name":"toolu","dependencies":[{"name":"caveman","marketplace":"caveman"}]}
JSON
  printf '%s' '{"plugins":{"caveman@caveman":{}}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "required plugins missing"
}

@test "dep-warning: suppressed entirely when registry is indeterminate (missing file)" {
  _manifest <<'JSON'
{"name":"toolu","dependencies":[{"name":"caveman","marketplace":"caveman"}]}
JSON
  rm -f "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "required plugins missing"
}

@test "dep-warning: no dependencies key is a silent no-op" {
  _manifest <<'JSON'
{"name":"toolu"}
JSON
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "required plugins missing"
}

@test "dep-warning: real toolu manifest (empty deps) emits no missing-plugin warning" {
  cp "$PLUGINS_DIR/toolu/.claude-plugin/plugin.json" "$PLUGROOT/.claude-plugin/plugin.json"
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "required plugins missing"
  ! echo "$output" | grep -q "caveman"
  ! echo "$output" | grep -q "code-simplifier"
}

@test "dep-warning: a nameless dependency entry is skipped (no null@ spec)" {
  _manifest <<'JSON'
{"name":"toolu","dependencies":[{"marketplace":"caveman"},{"name":"caveman","marketplace":"caveman"}]}
JSON
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "/plugin install caveman@caveman"
  ! echo "$output" | grep -q "null@"
}

@test "dep-warning: a malformed scalar entry does not suppress warnings for valid deps" {
  _manifest <<'JSON'
{"name":"toolu","dependencies":[42,{"name":"caveman","marketplace":"caveman"}]}
JSON
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "/plugin install caveman@caveman"
}
