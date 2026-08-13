#!/usr/bin/env bats
# Tests for hooks/post-tools/modules/gate-status.sh

HOOK="${BATS_TEST_DIRNAME}/../gate-status.sh"

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
  GATE_FILE="$TMP/.claude/tmp/quality-gate-status.json"
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Build a PostToolUse Bash payload for a command + exit code.
# Usage: _payload "cargo clippy" 0
_payload() {
  jq -n --arg cmd "$1" --argjson code "$2" \
    '{tool_input: {command: $cmd}, tool_response: {metadata: {exit_code: $code}}}'
}

# Seed the gate file with a given source + status.
# Usage: _write_gate "rust-quality-hook" "failing"
_write_gate() {
  mkdir -p "$TMP/.claude/tmp"
  jq -n --arg source "$1" --arg status "$2" \
    '{status: $status, source: $source, reason: "seeded by test", updatedAt: "2026-01-01T00:00:00Z"}' \
    > "$GATE_FILE"
}

@test "gate-status: non-Bash tool is a no-op (no gate file created)" {
  payload=$(_payload "cargo test" 0)
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}

@test "gate-status: non-quality command does not create a gate file" {
  payload=$(_payload "ls -la" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}

@test "gate-status: successful quality command writes passing gate when none exists" {
  payload=$(_payload "cargo clippy" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -f "$GATE_FILE" ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: failed quality command writes failing gate and emits context" {
  payload=$(_payload "bun test" 1)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.source == "gate-status-hook"' "$GATE_FILE"
  echo "$output" | grep -q "Global quality gate failing"
}

@test "gate-status: Codex Bash failure uses the Codex project state path" {
  payload=$(jq -n --arg cwd "$TMP" \
    '{session_id:"session-1",turn_id:"turn-1",cwd:$cwd,
      hook_event_name:"PostToolUse",tool_name:"Bash",
      tool_input:{command:"bun test"},
      tool_response:{metadata:{exit_code:1},stdout:"",stderr:"failed"}}')

  TOOLU_HOST_OVERRIDE=codex tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" \
    run bash "$HOOK"

  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$TMP/.codex/tmp/quality-gate-status.json"
  [ ! -e "$TMP/.claude/tmp/quality-gate-status.json" ]
}

@test "gate-status: failing rust-quality-hook gate survives unrelated successful quality command" {
  _write_gate "rust-quality-hook" "failing"
  payload=$(_payload "cargo clippy" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.source == "rust-quality-hook"' "$GATE_FILE"
}

@test "gate-status: failing ts-quality-hook gate survives unrelated successful quality command" {
  _write_gate "ts-quality-hook" "failing"
  payload=$(_payload "bun run check" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.source == "ts-quality-hook"' "$GATE_FILE"
}

@test "gate-status: failing gate-status-hook gate flips to passing when quality command passes" {
  # Drive the round-trip through the hook so the failing slot is created the same
  # way production does (gate_record_failure), then cleared by a passing command.
  failing=$(_payload "bun test" 1)
  tool_name=Bash input="$failing" PROJECT_ROOT="$TMP" run bash "$HOOK"
  jq -e '.status == "failing"' "$GATE_FILE"
  passing=$(_payload "cargo test" 0)
  tool_name=Bash input="$passing" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

# --- Silent-failure window (round-10 should-fix) ----------------------------
# A failing quality command must be recorded as its OWN slot even while a
# file-level hook owns the gate, so fixing the file later cannot flip the gate
# to passing while the command is still broken. A passing command, conversely,
# clears only its own slot and leaves the file failure intact.

# Seed an entries-format ts-quality-hook failure, the way the real hook writes it.
_seed_ts_entry() {
  mkdir -p "$TMP/.claude/tmp"
  jq -n '{status:"failing", reason:"bad ts", source:"ts-quality-hook",
          file:"/p/a.ts", violations:"viol\n",
          entries:{"/p/a.ts":{source:"ts-quality-hook", reason:"bad ts",
                              violations:"viol\n", updatedAt:"2026-01-01T00:00:00Z"}},
          updatedAt:"2026-01-01T00:00:00Z"}' > "$GATE_FILE"
}

@test "gate-status: failing quality command is recorded even while a file hook owns the gate" {
  _seed_ts_entry
  payload=$(_payload "bun test" 1)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.entries["/p/a.ts"].source == "ts-quality-hook"' "$GATE_FILE"
  jq -e '.entries["__global__"].source == "gate-status-hook"' "$GATE_FILE"
}

@test "gate-status: passing quality command clears only its slot, leaving a file-hook failure intact" {
  _seed_ts_entry
  payload=$(_payload "cargo test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.entries["/p/a.ts"].source == "ts-quality-hook"' "$GATE_FILE"
  jq -e '.entries | has("__global__") | not' "$GATE_FILE"
}

@test "gate-status: passing quality-hook gate can be overwritten by failing quality command" {
  _write_gate "ts-quality-hook" "passing"
  payload=$(_payload "bun test" 1)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE_FILE"
  jq -e '.source == "gate-status-hook"' "$GATE_FILE"
}

# --- Trigger-regex anchoring (item 1) ---------------------------------------

# Commands that SHOULD toggle the gate (genuine quality commands).
@test "gate-status: TRIGGER cargo test" {
  payload=$(_payload "cargo test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER leading-whitespace cargo clippy" {
  payload=$(_payload "  cargo clippy" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER cargo test after && in compound command" {
  payload=$(_payload "cd crate && cargo test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER bun test" {
  payload=$(_payload "bun test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER bun run check" {
  payload=$(_payload "bun run check" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER bun test after && in compound command" {
  payload=$(_payload "pnpm i && bun test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

# Lock-in: the `|` in GATE_TRIGGER_PREFIX is by design — a quality command
# that runs after a pipe still executed, so it must trigger gate registration.
# This guards against a future cleanup silently dropping `|` from the prefix.
@test "gate-status: TRIGGER cargo test after a pipe" {
  payload=$(_payload "find . | cargo test" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER tsc after a pipe" {
  payload=$(_payload "foo | tsc" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER bare tsc" {
  payload=$(_payload "tsc" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

@test "gate-status: TRIGGER tsc --noEmit" {
  payload=$(_payload "tsc --noEmit" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE_FILE"
}

# Commands that must NOT toggle the gate (substring false positives).
@test "gate-status: NO-TRIGGER cat tsconfig.json (was matching tsc)" {
  payload=$(_payload "cat tsconfig.json" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}

@test "gate-status: NO-TRIGGER ls tooling/foo/test.sh (substring path)" {
  payload=$(_payload "ls tooling/foo/test.sh" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}

@test "gate-status: NO-TRIGGER vitests-helper (no word boundary)" {
  payload=$(_payload "vitests-helper" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}

@test "gate-status: NO-TRIGGER cattsc (embedded substring)" {
  payload=$(_payload "cattsc" 0)
  tool_name=Bash input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ ! -f "$GATE_FILE" ]
}
