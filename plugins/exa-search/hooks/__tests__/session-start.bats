#!/usr/bin/env bats
# session-start.sh publishes the exa-search wrapper at a stable path
# the agent's Bash tool can reach without $CLAUDE_PLUGIN_ROOT.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/exa-search/scripts" && pwd)/search.sh"
}

teardown() { rm -rf "$TMP"; }

@test "session-start: publishes wrapper symlink at \$CLAUDE_CONFIG_DIR/exa-search/search.sh" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/exa-search/search.sh"
  [ -L "$dst" ]
  [ "$(readlink "$dst")" = "$SRC" ]
  [ -x "$dst" ]
}

@test "session-start: refreshes a stale symlink to the current target" {
  pub_root="$CLAUDE_CONFIG_DIR/exa-search"
  mkdir -p "$pub_root"
  ln -s /nonexistent/old/search.sh "$pub_root/search.sh"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$pub_root/search.sh")" = "$SRC" ]
}

@test "session-start: NEVER clobbers a real file at the wrapper path" {
  pub_root="$CLAUDE_CONFIG_DIR/exa-search"
  mkdir -p "$pub_root"
  printf '#!/usr/bin/env bash\necho user-override\n' > "$pub_root/search.sh"
  chmod +x "$pub_root/search.sh"
  before=$(cat "$pub_root/search.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -L "$pub_root/search.sh" ]
  [ "$(cat "$pub_root/search.sh")" = "$before" ]
}

@test "session-start: emits nothing on stdout (SessionStart hygiene)" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "session-start: idempotent (second run leaves the symlink identical)" {
  bash "$HOOK" <<<'{}'
  before=$(readlink "$CLAUDE_CONFIG_DIR/exa-search/search.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(readlink "$CLAUDE_CONFIG_DIR/exa-search/search.sh")
  [ "$before" = "$after" ]
}
