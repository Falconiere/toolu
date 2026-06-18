#!/usr/bin/env bats
# Tests for debug-stack.sh — driven against REAL captured node + cargo backtraces
# (see fixtures/debug/PROVENANCE.md). No synthetic stack output.

setup() {
  SCRIPT="$BATS_TEST_DIRNAME/../debug-stack.sh"
  FX="$BATS_TEST_DIRNAME/fixtures/debug"
}

@test "js: surfaces inner/middle/outer app frames with a throw.mjs location" {
  run "$SCRIPT" --file "$FX/stacktrace.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"inner"* ]]
  [[ "$output" == *"middle"* ]]
  [[ "$output" == *"outer"* ]]
  [[ "$output" == *"throw.mjs:1:25"* ]]
}

@test "js: node:internal frames are collapsed, not listed as app frames" {
  run "$SCRIPT" --file "$FX/stacktrace.txt"
  [ "$status" -eq 0 ]
  [[ "$output" != *"node:internal"* ]]
  [[ "$output" == *"framework/runtime frames collapsed"* ]]
}

@test "rust: surfaces level_three/level_two/level_one app frames" {
  run "$SCRIPT" --file "$FX/rust-panic.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"level_three"* ]]
  [[ "$output" == *"level_two"* ]]
  [[ "$output" == *"level_one"* ]]
}

@test "rust: core::panicking and __rustc frames are collapsed, not listed" {
  run "$SCRIPT" --file "$FX/rust-panic.txt"
  [ "$status" -eq 0 ]
  [[ "$output" != *"core::panicking"* ]]
  [[ "$output" != *"__rustc"* ]]
  [[ "$output" == *"framework/runtime frames collapsed"* ]]
}

@test "language-agnostic: both fixtures produce an APP FRAMES section" {
  run "$SCRIPT" --file "$FX/stacktrace.txt"; [[ "$output" == *"APP FRAMES"* ]]
  run "$SCRIPT" --file "$FX/rust-panic.txt"; [[ "$output" == *"APP FRAMES"* ]]
}

@test "json mode emits recognized:true with a non-empty app_frames" {
  run "$SCRIPT" --json --file "$FX/rust-panic.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"recognized":true'* ]]
  [[ "$output" == *'"app_frames":["panicker::level_three'* ]]
}

@test "DEBUG_MAX_FRAMES cap truncates and marks overflow" {
  DEBUG_MAX_FRAMES=1 run "$SCRIPT" --file "$FX/rust-panic.txt"
  [ "$status" -eq 0 ]
  [[ "$output" == *"more)"* ]]
}

@test "unrecognized input falls back to capped raw passthrough, exit 0" {
  run bash -c "printf 'just some text\n' | '$SCRIPT'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"no recognizable"* ]]
  [[ "$output" == *"just some text"* ]]
}

@test "bad argument exits non-zero" {
  run "$SCRIPT" --bogus
  [ "$status" -eq 2 ]
}

@test "unreadable --file exits non-zero" {
  run "$SCRIPT" --file "$FX/does-not-exist.txt"
  [ "$status" -eq 2 ]
}
