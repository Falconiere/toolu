#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SCRIPT="$ROOT/tooling/codex-smoke.sh"

@test "Codex installs, lists, exercises, and removes every local plugin" {
  command -v codex >/dev/null 2>&1 || skip "codex CLI is not installed"

  run bash "$SCRIPT"

  [ "$status" -eq 0 ]
  [[ "$output" == *"available=13"* ]]
  [[ "$output" == *"installed=13"* ]]
  [[ "$output" == *"session-start=17"* ]]
  [[ "$output" == *"removed=13"* ]]
}
