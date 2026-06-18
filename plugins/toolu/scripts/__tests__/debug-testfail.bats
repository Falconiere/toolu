#!/usr/bin/env bats
# Tests for debug-testfail.sh — driven against REAL captured bun + cargo failure
# transcripts (see fixtures/debug/PROVENANCE.md). No synthetic test output.

setup() {
  SCRIPT="$BATS_TEST_DIRNAME/../debug-testfail.sh"
  FX="$BATS_TEST_DIRNAME/fixtures/debug"
}

@test "bun: surfaces both failed test names" {
  run "$SCRIPT" --file "$FX/bun-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"add sums two numbers"* ]]
  [[ "$output" == *"add handles zero"* ]]
}

@test "bun: surfaces the assertion error and a file:line location" {
  run "$SCRIPT" --file "$FX/bun-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"expect(received).toBe(expected)"* ]]
  [[ "$output" == *"math.test.ts:4:21"* ]]
}

@test "cargo: surfaces the failed test name with the test prefix stripped" {
  run "$SCRIPT" --file "$FX/cargo-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"tests::adds"* ]]
  [[ "$output" != *"test tests::adds"* ]]
}

@test "cargo: surfaces the panic location src/lib.rs:6:17" {
  run "$SCRIPT" --file "$FX/cargo-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"src/lib.rs:6:17"* ]]
}

@test "json mode emits recognized:true with the location" {
  run "$SCRIPT" --json --file "$FX/cargo-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"recognized":true'* ]]
  [[ "$output" == *'"src/lib.rs:6:17"'* ]]
}

@test "language-agnostic: same script parses both TS and Rust without flags" {
  run "$SCRIPT" --file "$FX/bun-testfail.txt"; [[ "$output" == *"FAILED TESTS"* ]]
  run "$SCRIPT" --file "$FX/cargo-testfail.txt"; [[ "$output" == *"FAILED TESTS"* ]]
}

@test "unrecognized input falls back to capped raw passthrough, exit 0" {
  run bash -c "printf 'hello\nworld\nnothing failed\n' | '$SCRIPT'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"no recognizable test failures"* ]]
  [[ "$output" == *"hello"* ]]
}

@test "DEBUG_MAX_FAILURES cap truncates and marks overflow" {
  DEBUG_MAX_FAILURES=1 run "$SCRIPT" --file "$FX/bun-testfail.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"more)"* ]]
}

@test "bad argument exits non-zero" {
  run "$SCRIPT" --bogus
  [ "$status" -eq 2 ]
}

@test "unreadable --file exits non-zero" {
  run "$SCRIPT" --file "$FX/does-not-exist.txt"
  [ "$status" -eq 2 ]
}
