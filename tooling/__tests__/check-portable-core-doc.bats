#!/usr/bin/env bats
# Real-data checks for tooling/check-portable-core-doc.sh (#205).

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
CHECK="$ROOT/tooling/check-portable-core-doc.sh"

setup() {
  chmod +x "$CHECK"
}

@test "portable-core doc checker passes on the committed doc" {
  run bash "$CHECK"
  [ "$status" -eq 0 ]
  [[ "$output" == *"ok"* ]]
}

@test "portable-core doc checker fails when a required heading is removed" {
  local tmp="$BATS_TEST_TMPDIR/portable-core.md"
  sed '/^## Pins$/d' "$ROOT/docs/portable-core.md" >"$tmp"
  run env PORTABLE_CORE_DOC="$tmp" bash "$CHECK"
  [ "$status" -ne 0 ]
  [[ "$output" == *"missing heading"* ]] || [[ "$stderr" == *"missing heading"* ]]
}

@test "portable-core doc checker fails on invalid classification token maybe-later" {
  local tmp="$BATS_TEST_TMPDIR/portable-core.md"
  awk '
    /^## Policy split/ {print; print "- `maybe-later`"; next}
    {print}
  ' "$ROOT/docs/portable-core.md" >"$tmp"
  run env PORTABLE_CORE_DOC="$tmp" bash "$CHECK"
  [ "$status" -ne 0 ]
  [[ "$output$stderr" == *"maybe-later"* ]]
}
