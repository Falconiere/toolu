#!/usr/bin/env bats
# Tests for tooling/bats-run.sh — the parallel-aware suite runner.
#
# The runner's DECISIONS (job count, parallel vs serial, which flags) are
# asserted through its print-plan mode. Running a parallel bats inside bats is
# not a meaningful test: the inner run inherits the outer one's harness state
# and misreports. One end-to-end case forces the serial path, where nesting is
# well-behaved, so the runner is still proven to actually run a suite.

RUNNER="${BATS_TEST_DIRNAME}/../bats-run.sh"

setup() {
  TMP=$(mktemp -d)
  mkdir -p "$TMP/suite" "$TMP/bin"
  # The fixture's test lines are assembled rather than written literally: bats
  # counts every ^@test line in THIS file when it computes how many tests to
  # expect, heredoc bodies included, and two phantom entries make an otherwise
  # green suite exit non-zero with "Executed N instead of expected N+2".
  {
    printf '#!/usr/bin/env bats\n'
    printf '@%s "fixture one" { true; }\n' test
    printf '@%s "fixture two" { true; }\n' test
  } > "$TMP/suite/fixture.bats"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# A PATH with the tools bats needs but WITHOUT GNU parallel.
_path_without_parallel() {
  local tool src
  for tool in bats bash env dirname basename mkdir rm cat sed grep cut sort tr \
              awk getconf uname mktemp find flock realpath readlink tput; do
    src=$(command -v "$tool" 2>/dev/null) && ln -sf "$src" "$TMP/bin/$tool"
  done
  printf '%s' "$TMP/bin"
}

@test "plans a parallel run when GNU parallel is available" {
  command -v parallel >/dev/null 2>&1 || skip "GNU parallel not installed"
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=4 bash "$RUNNER" "$TMP/suite"
  [ "$status" -eq 0 ]
  [[ "$output" == *"--jobs 4"* ]]
  [[ "$output" == *"--no-parallelize-within-files"* ]]
}

@test "announces what it is doing on stderr" {
  command -v parallel >/dev/null 2>&1 || skip "GNU parallel not installed"
  run env BATS_RUN_PRINT_PLAN=1 bash "$RUNNER" "$TMP/suite"
  [[ "$output" == *"files in parallel"* ]]
  [[ "$output" == *"tests within a file serial"* ]]
}

@test "BATS_JOBS=1 plans a serial run" {
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=1 bash "$RUNNER" "$TMP/suite"
  [ "$status" -eq 0 ]
  [[ "$output" != *"--jobs"* ]]
}

@test "a non-numeric BATS_JOBS plans a serial run rather than passing it through" {
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=many bash "$RUNNER" "$TMP/suite"
  [ "$status" -eq 0 ]
  [[ "$output" != *"--jobs"* ]]
  [[ "$output" != *"many"* ]]
}

@test "falls back to serial with a hint when parallel is missing" {
  local path
  path=$(_path_without_parallel)
  run env PATH="$path" BATS_RUN_PRINT_PLAN=1 bash "$RUNNER" "$TMP/suite"
  [ "$status" -eq 0 ]
  [[ "$output" == *"GNU parallel not found"* ]]
  [[ "$output" == *"running serially"* ]]
  [[ "$output" != *"--jobs"* ]]
}

@test "defaults to the repo suite paths when none are given" {
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=1 bash "$RUNNER"
  [[ "$output" == *"bats -r plugins benchmarks tooling"* ]]
}

@test "the default covers every directory holding suites" {
  # A suite directory missing from the default is a suite nobody runs locally.
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=1 bash "$RUNNER"
  for dir in plugins benchmarks tooling; do
    [[ "$output" == *"$dir"* ]]
  done
}

@test "a path with no suites fails loudly rather than confusingly" {
  mkdir -p "$TMP/empty"
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=1 bash "$RUNNER" "$TMP/empty"
  [ "$status" -ne 0 ]
  [[ "$output" == *"test discovery is broken"* ]]
}

@test "the given paths are what it plans to run" {
  run env BATS_RUN_PRINT_PLAN=1 BATS_JOBS=1 bash "$RUNNER" "$TMP/suite"
  [[ "$output" == *"$TMP/suite"* ]]
}

# No test here runs a suite THROUGH the runner: bats does not nest — an inner
# run inherits the outer harness's state and both misreport (the outer run ends
# with "Executed 9 instead of expected 12"). The runner's end-to-end proof is
# that `bun run test` drives the entire repo suite through it, so every full
# run exercises it for real.
