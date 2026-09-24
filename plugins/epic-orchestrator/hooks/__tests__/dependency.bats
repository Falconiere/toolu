#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
SCRIPT="$ROOT/plugins/epic-orchestrator/hooks/check-deps.sh"

setup() {
  command -v codex >/dev/null 2>&1 || skip "codex CLI is not installed"
  CODEX_TEST_HOME="$BATS_TEST_TMPDIR/codex"
  export CODEX_HOME="$CODEX_TEST_HOME"
  mkdir -p "$CODEX_HOME"
  codex plugin marketplace add "$ROOT" --json >/dev/null
}

@test "warns when toolu and pr-babysit are missing" {
  bin="$BATS_TEST_TMPDIR/bin"
  mkdir -p "$bin"
  printf '%s\n' '#!/usr/bin/env bash' 'printf "%s\n" "{\"installed\":[]}"' >"$bin/codex"
  chmod +x "$bin/codex"

  run env PATH="$bin:$PATH" PLUGIN_ROOT="$ROOT/plugins/epic-orchestrator" bash "$SCRIPT"

  [ "$status" -eq 0 ]
  [ "$(jq -r '.hookSpecificOutput.hookEventName' <<<"$output")" = SessionStart ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"codex plugin add toolu@toolu"* ]]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"codex plugin add pr-babysit@toolu"* ]]
}

@test "silent when toolu and pr-babysit are installed" {
  bin="$BATS_TEST_TMPDIR/bin"
  mkdir -p "$bin"
  printf '%s\n' '#!/usr/bin/env bash' \
    'printf "%s\n" "{\"installed\":[{\"pluginId\":\"toolu@toolu\",\"installed\":true,\"enabled\":true},{\"pluginId\":\"pr-babysit@toolu\",\"installed\":true,\"enabled\":true}]}"' \
    >"$bin/codex"
  chmod +x "$bin/codex"

  run env PATH="$bin:$PATH" PLUGIN_ROOT="$ROOT/plugins/epic-orchestrator" bash "$SCRIPT"

  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "hooks.json points at check-deps.sh" {
  jq -e --arg command '${CLAUDE_PLUGIN_ROOT}/hooks/check-deps.sh' '
    any(.hooks.SessionStart[].hooks[]; .command == $command)
  ' "$ROOT/plugins/epic-orchestrator/hooks/hooks.json" >/dev/null
  [ -x "$SCRIPT" ]
}
