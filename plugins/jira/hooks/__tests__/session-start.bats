#!/usr/bin/env bats
# session-start.sh publishes the jira wrapper at a stable path the agent's
# Bash tool can reach without $CLAUDE_PLUGIN_ROOT.

# `run --separate-stderr` is a 1.5.0+ flag.
bats_require_minimum_version 1.5.0

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../dist" && pwd)/jira.js"
}

teardown() { rm -rf "$TMP"; }

@test "session-start: publishes wrapper symlink at \$CLAUDE_CONFIG_DIR/jira/jira.sh" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/jira/jira.sh"
  [ -L "$dst" ]
  [ "$(readlink "$dst")" = "$SRC" ]
  [ -x "$dst" ]
}

@test "session-start: refreshes a stale symlink to the current target" {
  pub_root="$CLAUDE_CONFIG_DIR/jira"
  mkdir -p "$pub_root"
  ln -s /nonexistent/old/jira.sh "$pub_root/jira.sh"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$pub_root/jira.sh")" = "$SRC" ]
}

@test "session-start: NEVER clobbers a real file at the wrapper path" {
  pub_root="$CLAUDE_CONFIG_DIR/jira"
  mkdir -p "$pub_root"
  printf '#!/usr/bin/env bash\necho user-override\n' > "$pub_root/jira.sh"
  chmod +x "$pub_root/jira.sh"
  before=$(cat "$pub_root/jira.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -L "$pub_root/jira.sh" ]
  [ "$(cat "$pub_root/jira.sh")" = "$before" ]
}

@test "session-start: emits nothing on stdout (SessionStart hygiene)" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "session-start: idempotent (second run leaves the symlink identical)" {
  bash "$HOOK" <<<'{}'
  before=$(readlink "$CLAUDE_CONFIG_DIR/jira/jira.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(readlink "$CLAUDE_CONFIG_DIR/jira/jira.sh")
  [ "$before" = "$after" ]
}

# Fail-soft: a corrupted install where hooks/dist/ is missing must NOT break the
# session. See context7's equivalent test for the rationale.
@test "session-start: source wrapper missing -> exits 0, no symlink, silent (fail-soft)" {
  fake="$TMP/fake-plugin/hooks"
  mkdir -p "$fake"
  cp "$HOOK" "$fake/session-start.sh"
  run bash "$fake/session-start.sh" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jira/jira.sh" ]
}

@test "session-start: the published path runs the TypeScript CLI bundle" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/jira/jira.sh"
  run "$dst"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira"* ]]
  [[ "$output" == *"plan         init|run|status|path"* ]]
}

@test "session-start: bun missing from PATH -> one-line advisory on stderr, still publishes, exits 0" {
  bash_bin="$(command -v bash)"
  run --separate-stderr env PATH=/usr/bin:/bin "$bash_bin" "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$stderr" = "jira: bun not found on PATH — the jira CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)" ]
  [ -L "$CLAUDE_CONFIG_DIR/jira/jira.sh" ]
}

@test "session-start: bun on PATH -> no advisory" {
  run --separate-stderr bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$stderr" ]
}
