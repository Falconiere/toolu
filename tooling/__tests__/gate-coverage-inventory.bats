#!/usr/bin/env bats
# Real-data checks for tooling/src/gate-coverage-inventory.ts (#209).

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
CLI="$ROOT/tooling/src/gate-coverage-inventory.ts"

@test "discover emits >=90 rows including protected-files, gate-mode, and a ts-quality concern" {
  run bun run "$CLI" discover
  [ "$status" -eq 0 ]
  local n
  n=$(printf '%s' "$output" | jq 'length')
  [ "$n" -ge 90 ]
  printf '%s' "$output" | jq -e 'map(.id) | any(contains("protected-files"))' >/dev/null
  printf '%s' "$output" | jq -e 'map(.id) | any(contains("gate-mode"))' >/dev/null
  printf '%s' "$output" | jq -e 'map(.id) | any(startswith("ts-quality:concern:"))' >/dev/null
}

@test "check passes on the committed inventory and matrix" {
  run bun run "$CLI" check
  [ "$status" -eq 0 ]
  [ "$output" = "gate-coverage-inventory: ok" ]
}

@test "check fails when inventory drops a discovered hooks.json id" {
  local tmp="$BATS_TEST_TMPDIR/inventory.json"
  local drop
  drop=$(jq -r '.[] | select(.kind=="hooks.json" and .plugin=="toolu" and (.commandOrModule|endswith("mod.sh")) and .event=="PreToolUse") | .id' \
    "$ROOT/tooling/fixtures/gate-coverage/inventory.json" | head -1)
  [ -n "$drop" ]
  jq --arg id "$drop" '[.[] | select(.id != $id)]' \
    "$ROOT/tooling/fixtures/gate-coverage/inventory.json" >"$tmp"
  local before after
  before=$(jq 'length' "$ROOT/tooling/fixtures/gate-coverage/inventory.json")
  after=$(jq 'length' "$tmp")
  [ "$after" -eq $((before - 1)) ]
  run env GATE_COVERAGE_INVENTORY="$tmp" bun run "$CLI" check
  [ "$status" -ne 0 ]
}

@test "check fails when matrix omits an inventory id" {
  local tmp="$BATS_TEST_TMPDIR/matrix.md"
  local id
  id=$(jq -r '.[0].id' "$ROOT/tooling/fixtures/gate-coverage/inventory.json")
  grep -vF "$id" "$ROOT/docs/gate-coverage-matrix.md" >"$tmp"
  run env GATE_COVERAGE_MATRIX="$tmp" bun run "$CLI" check
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" "$stderr" | grep -Fq "matrix missing id: $id"
}

@test "check fails on invalid classification maybe-later" {
  local tmp="$BATS_TEST_TMPDIR/inventory.json"
  jq '.[0].classification = "maybe-later"' "$ROOT/tooling/fixtures/gate-coverage/inventory.json" >"$tmp"
  run env GATE_COVERAGE_INVENTORY="$tmp" bun run "$CLI" check
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" "$stderr" | grep -Fq "invalid classification maybe-later"
}

@test "check fails when support=n/a pairs with shell-out" {
  local tmp="$BATS_TEST_TMPDIR/inventory.json"
  jq '.[0].classification = "shell-out" | .[0].support = "n/a" | .[0].implementationIssue = 210' \
    "$ROOT/tooling/fixtures/gate-coverage/inventory.json" >"$tmp"
  run env GATE_COVERAGE_INVENTORY="$tmp" bun run "$CLI" check
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" "$stderr" | grep -Fq "support=n/a requires classification=no-map"
}
