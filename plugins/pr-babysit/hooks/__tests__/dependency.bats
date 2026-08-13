#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
SCRIPT="$ROOT/plugins/pr-babysit/hooks/check-toolu.sh"

setup() {
  command -v codex >/dev/null 2>&1 || skip "codex CLI is not installed"
  CODEX_TEST_HOME="$BATS_TEST_TMPDIR/codex"
  export CODEX_HOME="$CODEX_TEST_HOME"
  mkdir -p "$CODEX_HOME"
  codex plugin marketplace add "$ROOT" --json >/dev/null
}

@test "dependent plugin warns with the exact Codex core install command" {
  codex plugin add pr-babysit@toolu --json >/dev/null

  run env PLUGIN_ROOT="$ROOT/plugins/pr-babysit" bash "$SCRIPT"

  [ "$status" -eq 0 ]
  [ "$(jq -r '.hookSpecificOutput.hookEventName' <<<"$output")" = SessionStart ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"codex plugin add toolu@toolu"* ]]
}

@test "dependency check does not wait for SessionStart stdin EOF" {
  codex plugin add pr-babysit@toolu --json >/dev/null
  fifo="$BATS_TEST_TMPDIR/session-start-input"

  run timeout 2 bash -c \
    'mkfifo "$1"; (sleep 10) >"$1" & holder=$!; trap '\''kill "$holder" 2>/dev/null || true; wait "$holder" 2>/dev/null || true'\'' EXIT; env PLUGIN_ROOT="$2" bash "$3" <"$1"' \
    _ "$fifo" "$ROOT/plugins/pr-babysit" "$SCRIPT"

  [ "$status" -eq 0 ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"codex plugin add toolu@toolu"* ]]
}

@test "dependency check is silent when toolu is installed" {
  codex plugin add toolu@toolu --json >/dev/null
  codex plugin add pr-babysit@toolu --json >/dev/null

  run env PLUGIN_ROOT="$ROOT/plugins/pr-babysit" bash "$SCRIPT"

  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "dependency check warns when the core record is disabled or not installed" {
  bin="$BATS_TEST_TMPDIR/bin"
  mkdir -p "$bin"
  for flags in \
    '"installed":false,"enabled":true' \
    '"installed":true,"enabled":false'; do
    printf '#!/usr/bin/env bash\nprintf '\''%%s\\n'\'' '\''{"installed":[{"pluginId":"toolu@toolu",%s}]} '\''\n' \
      "$flags" > "$bin/codex"
    chmod +x "$bin/codex"

    run env PATH="$bin:$PATH" PLUGIN_ROOT="$ROOT/plugins/pr-babysit" \
      bash "$SCRIPT"

    [ "$status" -eq 0 ]
    [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"codex plugin add toolu@toolu"* ]]
  done
}

@test "all core-dependent plugins ship the same self-contained check" {
  for plugin in comemory rust-quality ts-quality pr-babysit; do
    file="$ROOT/plugins/$plugin/hooks/check-toolu.sh"
    [ -x "$file" ]
    cmp "$SCRIPT" "$file"
    jq -e --arg command '${CLAUDE_PLUGIN_ROOT}/hooks/check-toolu.sh' '
      any(.hooks.SessionStart[].hooks[]; .command == $command)
    ' "$ROOT/plugins/$plugin/hooks/hooks.json" >/dev/null
  done
}
