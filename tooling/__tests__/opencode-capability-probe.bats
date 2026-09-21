#!/usr/bin/env bats
# Real-data checks for tooling/opencode-capability-probe.ts (#205).

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
PROBE="$ROOT/tooling/opencode-capability-probe.ts"

@test "capability probe fixture mode passes against committed doc+fixture" {
  run env PORTABLE_CORE_PROBE_MODE=fixture bun run "$PROBE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"fixture ok"* ]]
}

@test "capability probe fails closed for nonexistent OPENCODE_BIN within 10s" {
  run env -u PORTABLE_CORE_PROBE_MODE OPENCODE_BIN=/nonexistent/opencode \
    bash -c 'bun run "'"$PROBE"'"'
  [ "$status" -ne 0 ]
  [[ "$output$stderr" != *"[Ee]nforced"* ]] || [[ "$output$stderr" != *"enforced success"* ]]
  # Explicit: must not claim success
  ! printf '%s\n' "$output$stderr" | grep -qi 'enforced'
}

@test "capability probe fails on pin mismatch when fake CLI prints wrong version" {
  local fake="$BATS_TEST_TMPDIR/fake-opencode"
  printf '#!/bin/sh\necho opencode v0.0.1\n' >"$fake"
  chmod +x "$fake"
  run env -u PORTABLE_CORE_PROBE_MODE OPENCODE_BIN="$fake" bun run "$PROBE"
  [ "$status" -ne 0 ]
  ! printf '%s\n' "$output$stderr" | grep -qi 'enforced'
}
