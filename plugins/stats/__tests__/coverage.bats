#!/usr/bin/env bats
# Coverage guard: every model that appears in the real fixtures must be priced by
# an explicit rate-table branch in pricing.sh, NOT by the unknown/Sonnet default.
# If a fixture introduces a new model the rate table doesn't know, it surfaces
# here as a failing test — a deliberate tripwire so a new model is noticed and
# priced, never silently defaulted.

setup() {
  export TZ=UTC
  source "${BATS_TEST_DIRNAME}/../scripts/lib/usage.sh"
  F="${BATS_TEST_DIRNAME}/fixtures"
}

@test "no model in any real fixture falls through to the unknown rate" {
  local f um
  for f in "$F"/*.jsonl; do
    um="$(stats_usage_rollup "$f" | jq -c '.unknown_models')"
    [ "$um" = "[]" ] || { echo "fixture $f priced an unknown model: $um"; return 1; }
  done
}

@test "subagent fixtures are also covered (known models only)" {
  local um
  um="$(stats_usage_rollup "$F/sub-session.jsonl" "$F"/sub-session/subagents/agent-*.jsonl \
        | jq -c '.unknown_models')"
  [ "$um" = "[]" ]
}
