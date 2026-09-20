#!/usr/bin/env bats
# docs.bats — the methodology contract is present and states the load-bearing
# rules (fair baseline, never-mix modes, live=usage, provenance).

setup() {
  BENCH_DIR="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
}

@test "benchmarks/README.md exists" {
  [ -f "$BENCH_DIR/README.md" ]
}

@test "results/README.md exists and states the methodology rules" {
  local f="$BENCH_DIR/results/README.md"
  [ -f "$f" ]
  grep -qi "methodology"        "$f"
  grep -q  "## Fair baseline"   "$f"   # the fair-baseline section is present
  grep -qi "pins its baseline"  "$f"   # and states the committed-prompt rule
  grep -qi "never mixed"        "$f"   # tokenizer modes
  grep -qi "message.usage"      "$f"   # live tiers use real usage
  grep -qi "provenance"         "$f"
}
