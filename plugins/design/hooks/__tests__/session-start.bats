#!/usr/bin/env bats
# Tests for the design SessionStart publish hook. It symlinks the detector
# (ln -sf) and the references/ directory (ln -sfn) into
# ${CLAUDE_CONFIG_DIR}/design, idempotently, without ever nesting
# references/references. Exercises the REAL plugin files (no mocks).

setup() {
  PLUGIN_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"   # .../plugins/design
  HOOK="$PLUGIN_ROOT/hooks/session-start.sh"
  REG="$(mktemp -d)"
}
teardown() { rm -rf "$REG"; }

# Run the hook with a sandboxed config dir; </dev/null so the hook's stdin
# drain reads EOF instead of the caller's input.
run_hook() {
  CLAUDE_CONFIG_DIR="$REG" CLAUDE_PLUGIN_ROOT="$PLUGIN_ROOT" run bash "$HOOK" </dev/null
}

@test "publishes detect-stack.sh as a symlink to the plugin source" {
  run_hook
  [ "$status" -eq 0 ]
  [ -L "$REG/design/detect-stack.sh" ]
  [ "$(readlink "$REG/design/detect-stack.sh")" = "$PLUGIN_ROOT/scripts/detect-stack.sh" ]
}

@test "publishes references/ as a dir symlink (motion.md readable through it)" {
  run_hook
  [ "$status" -eq 0 ]
  [ -L "$REG/design/references" ]
  [ "$(readlink "$REG/design/references")" = "$PLUGIN_ROOT/references" ]
  [ -e "$REG/design/references/motion.md" ]
}

@test "re-run is idempotent and never nests references/references" {
  run_hook; [ "$status" -eq 0 ]
  run_hook; [ "$status" -eq 0 ]
  [ -L "$REG/design/references" ]
  [ ! -e "$REG/design/references/references" ]
}
