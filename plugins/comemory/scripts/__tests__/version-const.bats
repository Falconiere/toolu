#!/usr/bin/env bats
# Drift guard: comemory's setup.sh mirrors toolu's COMEMORY_MIN_VERSION (the two
# live in sibling plugins with no stable runtime path between them, so the value
# is hand-mirrored, not sourced). This test fails if they drift, so a bump in
# one is forced into the other. Both declare the SAME constant NAME so this
# guard can match by name.

ROOT="${BATS_TEST_DIRNAME}/../../../.."   # scripts/__tests__ -> scripts -> comemory -> plugins -> repo root

@test "COMEMORY_MIN_VERSION is in sync across toolu/detect.sh and comemory/setup.sh" {
  detect="$ROOT/plugins/toolu/hooks/lib/detect.sh"
  setup="$ROOT/plugins/comemory/scripts/setup.sh"
  [ -f "$detect" ]
  [ -f "$setup" ]
  d=$(grep -E '^COMEMORY_MIN_VERSION=' "$detect" | head -1)
  s=$(grep -E '^COMEMORY_MIN_VERSION=' "$setup" | head -1)
  [ -n "$d" ]
  [ -n "$s" ]
  [ "$d" = "$s" ]
}
