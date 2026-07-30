#!/usr/bin/env bats
# common.bats — the benchmarks bootstrap resolves the repo root and exposes
# benchmarks' own token-math functions. Real repo, no mocks.

setup() {
  BENCH_LIB="$BATS_TEST_DIRNAME/../lib"
}

@test "common.sh sources cleanly and BENCH_ROOT is the git toplevel" {
  source "$BENCH_LIB/common.sh"
  [ -n "$BENCH_ROOT" ]
  [ "$BENCH_ROOT" = "$(git -C "$BENCH_LIB" rev-parse --show-toplevel)" ]
}

@test "common.sh exposes stats_usage_rollup (reused, not reimplemented)" {
  source "$BENCH_LIB/common.sh"
  declare -F stats_usage_rollup
}

@test "common.sh exposes stats_pricing_jq and STATS_PRICING_ID" {
  source "$BENCH_LIB/common.sh"
  declare -F stats_pricing_jq
  [ -n "$STATS_PRICING_ID" ]
}

@test "BENCH_PRICING_LIB points at benchmarks' own pricing/usage lib dir" {
  source "$BENCH_LIB/common.sh"
  [ -f "$BENCH_PRICING_LIB/usage.sh" ]
  [ -f "$BENCH_PRICING_LIB/pricing.sh" ]
}
