#!/usr/bin/env bats
# Tests for debug-log.sh — driven against the REAL ~3000-line / ~120KB captured log
# (see fixtures/debug/big.log: cargo+bun verbose output + git log -p). No synthetic input.

setup() {
  SCRIPT="$BATS_TEST_DIRNAME/../debug-log.sh"
  FX="$BATS_TEST_DIRNAME/fixtures/debug"
  BIG="$FX/big.log"
}

@test "big.log: status 0 and output line count <= DEBUG_MAX_LINES (default 100)" {
  run "$SCRIPT" --file "$BIG"
  [ "$status" -eq 0 ]
  [ "${#lines[@]}" -le 100 ]
}

@test "big.log: output byte size <= DEBUG_MAX_BYTES (default 65536)" {
  run "$SCRIPT" --file "$BIG"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | wc -c)" -le 65536 ]
}

@test "big.log: reports a TOTAL lines marker in the thousands" {
  run "$SCRIPT" --file "$BIG"
  [ "$status" -eq 0 ]
  [[ "$output" == *"TOTAL lines: "* ]]
  [[ "$output" =~ TOTAL\ lines:\ [0-9]{4} ]]
}

@test "DEBUG_MAX_LINES=10 caps output to <= 10 lines" {
  DEBUG_MAX_LINES=10 run "$SCRIPT" --file "$BIG"
  [ "$status" -eq 0 ]
  [ "${#lines[@]}" -le 10 ]
}

@test "json mode on big.log: truncated:true with a total_lines field" {
  run "$SCRIPT" --json --file "$BIG"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"truncated":true'* ]]
  [[ "$output" == *'"total_lines":'* ]]
}

@test "dedup: identical ERROR lines collapse to one in the errors section" {
  run bash -c "printf 'ERROR boom\nERROR boom\nERROR boom\n' | '$SCRIPT'"
  [ "$status" -eq 0 ]
  # Scope the count to the ERRORS section (header .. TAIL header); dedup applies there.
  errsec="$(printf '%s\n' "$output" | awk '/^ERRORS\/WARNINGS/{f=1;next} /^TAIL /{f=0} f')"
  [ "$(printf '%s\n' "$errsec" | grep -c 'ERROR boom')" -eq 1 ]
}

@test "empty input is valid and exits 0" {
  run bash -c "printf '' | '$SCRIPT'"
  [ "$status" -eq 0 ]
}

@test "bad argument exits non-zero" {
  run "$SCRIPT" --bogus
  [ "$status" -eq 2 ]
}

@test "unreadable --file exits non-zero" {
  run "$SCRIPT" --file "$FX/does-not-exist.log"
  [ "$status" -eq 2 ]
}

@test "big.log: completes without hanging (fast path, status 0)" {
  run "$SCRIPT" --file "$BIG"
  [ "$status" -eq 0 ]
}
