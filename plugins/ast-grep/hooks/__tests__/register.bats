#!/usr/bin/env bats
# register.sh syncs this plugin's pre-tools.d modules into the toolu
# runtime registry as <spec>__<name>.sh, prunes its own stale entries, and
# never touches other plugins' files.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/register.sh"
  SRC_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../pre-tools.d" && pwd)"
}

teardown() { rm -rf "$TMP"; }

@test "register: syncs every module into pre-tools.d with the spec prefix" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  for src in "$SRC_DIR"/*.sh; do
    name=$(basename "$src")
    dst="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d/ast-grep@toolu__${name}"
    [ -f "$dst" ]
    cmp -s "$src" "$dst"
  done
}

@test "register: Codex PLUGIN_ROOT takes precedence and writes under CODEX_HOME" {
  local codex_home="$TMP/codex-home"
  local isolated_home="$TMP/home"
  run env -u CLAUDE_CONFIG_DIR HOME="$isolated_home" PLUGIN_ROOT="$(cd "$(dirname "$REGISTER")/.." && pwd)" CODEX_HOME="$codex_home" \
    bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ -f "$codex_home/toolu/pre-tools.d/ast-grep@toolu__search-nudge.sh" ]
  [ ! -e "$isolated_home/.claude/toolu/pre-tools.d/ast-grep@toolu__search-nudge.sh" ]
}

@test "register: prunes its own stale entries but not other plugins'" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  echo '#!/usr/bin/env bash' > "$regdir/ast-grep@toolu__removed-module.sh"
  echo '#!/usr/bin/env bash' > "$regdir/other@market__keep.sh"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -f "$regdir/ast-grep@toolu__removed-module.sh" ]
  [ -f "$regdir/other@market__keep.sh" ]
}

@test "register: refreshes a registry copy that drifted from source" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  echo 'stale content' > "$regdir/ast-grep@toolu__search-nudge.sh"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  cmp -s "$SRC_DIR/search-nudge.sh" "$regdir/ast-grep@toolu__search-nudge.sh"
}

@test "register: idempotent (second run changes nothing, exits 0)" {
  bash "$REGISTER" <<<'{}'
  before=$(ls "$CLAUDE_CONFIG_DIR/toolu/pre-tools.d" | sort)
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(ls "$CLAUDE_CONFIG_DIR/toolu/pre-tools.d" | sort)
  [ "$before" = "$after" ]
}

@test "register: syncs post-tools.d modules with the spec prefix" {
  POST_SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../post-tools.d" && pwd)"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  for src in "$POST_SRC"/*.sh; do
    name=$(basename "$src")
    dst="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/ast-grep@toolu__${name}"
    [ -f "$dst" ]
    cmp -s "$src" "$dst"
  done
}

@test "register e2e: post-tools module records a byte-savings ledger via the core dispatcher" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/post-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{"ast-grep@toolu":{}}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  f="$TMP/big.txt"; printf 'x%.0s' {1..4000} > "$f"
  payload=$(jq -n --arg fp "$f" '{tool_name:"Read",session_id:"e2e",tool_input:{file_path:$fp},tool_response:"abc"}')
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<"$payload"
  [ "$status" -eq 0 ]
  led="$CLAUDE_CONFIG_DIR/toolu/byte-savings/e2e.jsonl"
  [ -f "$led" ]
  [ "$(jq -r '.kind'     "$led")" = "read" ]
  [ "$(jq -r '.returned' "$led")" = "3" ]
  [ "$(jq -r '.full'     "$led")" = "4000" ]
}

@test "register e2e: post-tools module is gated off when the plugin is absent" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/post-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  f="$TMP/big.txt"; printf 'x%.0s' {1..4000} > "$f"
  payload=$(jq -n --arg fp "$f" '{tool_name:"Read",session_id:"gated",tool_input:{file_path:$fp},tool_response:"abc"}')
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<"$payload"
  [ "$status" -eq 0 ]
  [ ! -e "$CLAUDE_CONFIG_DIR/toolu/byte-savings/gated.jsonl" ]
}

@test "register: writes are atomic (no *.tmp.* leftovers)" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  found=$(find "$CLAUDE_CONFIG_DIR/toolu" -name '*.tmp.*' | wc -l | tr -d ' ')
  [ "$found" = "0" ]
}

@test "register: emits nothing on stdout (SessionStart hygiene)" {
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "register e2e: synced modules execute through the core dispatcher when installed" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/pre-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{"ast-grep@toolu":{}}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<'{"tool_name":"Grep","tool_input":{"pattern":"fn handle_request","glob":"*.rs"}}'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("ast-grep")' >/dev/null
}

@test "register e2e: synced modules are gated off when the plugin is definitively absent" {
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/pre-tools" && pwd)/mod.sh"
  bash "$REGISTER" <<<'{}'
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$CLAUDE_CONFIG_DIR" HOME="$TMP" \
    bash "$CORE_MOD" <<<'{"tool_name":"Grep","tool_input":{"pattern":"fn handle_request","glob":"*.rs"}}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "register: clears its own AGED orphaned tmp residue, keeps fresh + foreign tmp" {
  regdir="$CLAUDE_CONFIG_DIR/toolu/pre-tools.d"
  mkdir -p "$regdir"
  # Aged orphan (ours): from a crashed run — must be removed.
  echo 'partial' > "$regdir/ast-grep@toolu__search-nudge.sh.tmp.12345"
  touch -t 202601010000 "$regdir/ast-grep@toolu__search-nudge.sh.tmp.12345"
  # Fresh tmp (ours): a concurrent SessionStart mid-write — must survive.
  echo 'partial' > "$regdir/ast-grep@toolu__fresh-module.sh.tmp.777"
  # Foreign tmp: never ours to touch, fresh or aged.
  echo 'partial' > "$regdir/other@market__keep.sh.tmp.99"
  touch -t 202601010000 "$regdir/other@market__keep.sh.tmp.99"
  run bash "$REGISTER" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -f "$regdir/ast-grep@toolu__search-nudge.sh.tmp.12345" ]
  [ -f "$regdir/ast-grep@toolu__fresh-module.sh.tmp.777" ]
  [ -f "$regdir/other@market__keep.sh.tmp.99" ]
}
