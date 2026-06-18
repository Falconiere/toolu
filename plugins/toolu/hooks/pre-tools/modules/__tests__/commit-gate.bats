#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/commit-gate.sh

HOOK="${BATS_TEST_DIRNAME}/../commit-gate.sh"

setup() {
  TMP=$(mktemp -d)
  export TOOLU_SETTINGS_DIR="$TMP/settings"
  mkdir -p "$TOOLU_SETTINGS_DIR"
  printf '%s\n' "feat" "fix" "chore" "docs" "refactor" "test" \
    > "$TOOLU_SETTINGS_DIR/commit-prefixes.txt"
}

teardown() {
  unset TOOLU_SETTINGS_DIR
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

run_hook() {
  local cmd="$1"
  local payload
  payload=$(jq -n --arg c "$cmd" '{tool_name:"Bash",tool_input:{command:$c}}')
  tool_name=Bash input="$payload" run bash "$HOOK" <<<"$payload"
}

@test "commit-gate: accepts 'feat:' prefix" {
  run_hook 'git commit -m "feat: add widget"'
  [ "$status" -eq 0 ]
  # Must NOT be denied — context message OK.
  ! echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null 2>&1
}

@test "commit-gate: rejects unknown prefix" {
  run_hook 'git commit -m "wibble: stuff"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

# Regression: `[^"]*` stopped at the first escaped quote, truncating the
# message and losing the prefix.
@test "commit-gate: accepts 'fix:' message containing escaped quotes" {
  run_hook 'git commit -m "fix: handle \"quoted\" text"'
  [ "$status" -eq 0 ]
  ! echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null 2>&1
  # Sanity: the gate still ran (context message emitted).
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext' >/dev/null
}

@test "commit-gate: rejects unknown prefix even with escaped quotes in message" {
  run_hook 'git commit -m "wibble: a \"b\" c"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

# Sharpest regression: escaped quote BEFORE the colon. The truncated message
# (`wibble(\`) lost the colon, so the bad prefix escaped validation entirely.
@test "commit-gate: rejects unknown prefix when scope contains escaped quotes" {
  run_hook 'git commit -m "wibble(\"ui\"): add stuff"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "commit-gate: accepts known prefix when scope contains escaped quotes" {
  run_hook 'git commit -m "feat(\"ui\"): add stuff"'
  [ "$status" -eq 0 ]
  ! echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null 2>&1
}

@test "commit-gate: accepts single-quoted -m message" {
  run_hook "git commit -m 'feat: single quoted'"
  [ "$status" -eq 0 ]
  ! echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null 2>&1
}
