#!/usr/bin/env bats
# session-start.sh publishes the jira wrapper at a stable path the agent's
# Bash tool can reach without $CLAUDE_PLUGIN_ROOT.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/jira/scripts" && pwd)/jira.sh"
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

# Fail-soft: a corrupted install where skills/ is missing must NOT break the
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
