#!/usr/bin/env bats
# billing.sh — attaches the dual API/subscription billing lens. Each test builds
# a real aggregate (via aggregate.sh) from a crafted single-session rollup dated
# TODAY (so it lands in the month-to-date window), then runs it through
# stats_attach_billing with plan/limit env set. bats isolates each test, and
# setup() clears the env, so exports don't leak between tests.

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/aggregate.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/billing.sh"
  TODAY="$(date +%Y-%m-%d)"
  unset STATS_PLAN STATS_BILLING STATS_WEEKLY_LIMIT STATS_WINDOW_LIMIT
}

# agg COST TOKENS -> aggregate JSON for one session, all usage dated today.
agg() {
  local c="$1" t="$2"
  local b="{\"tokens\":$t,\"input\":$t,\"output\":0,\"cache_read\":0,\"cache_write\":0,\"cost\":$c}"
  printf '[{"session_id":"s","project":"p","project_path":"/p","totals":%s,"by_day":{"%s":%s},"by_model":{},"tools":{},"phases":{}}]' \
    "$b" "$TODAY" "$b" | stats_aggregate
}
f() { jq -r "$1" <<<"$OUT"; }   # field of the last billing OUT

@test "subscription: api-equivalent value and savings on a month basis" {
  export STATS_PLAN=max5
  OUT="$(agg 250 1000000 | stats_attach_billing)"
  [ "$(f '.billing.plan_fee_monthly')" = "100" ]
  [ "$(f '.billing.subscription.api_equivalent_value')" = "250" ]
  [ "$(f '.billing.subscription.savings')" = "150" ]
}

@test "subscription: effective rate * month-Mtok equals the flat fee exactly" {
  export STATS_PLAN=max5
  OUT="$(agg 250 5000000 | stats_attach_billing)"
  [ "$(f '.billing.subscription.effective_per_mtok')" = "20" ]            # 100 / 5 Mtok
  [ "$(f '(.billing.subscription.effective_per_mtok * 5) == .billing.plan_fee_monthly')" = "true" ]
}

@test "subscription: under-using shows NEGATIVE savings, not hidden" {
  export STATS_PLAN=pro
  OUT="$(agg 12 1000 | stats_attach_billing)"
  [ "$(f '.billing.subscription.savings')" = "-8" ]                       # 12 - 20
}

@test "quota: no percentage without a configured ceiling (honest)" {
  OUT="$(agg 5 3100000 | stats_attach_billing)"
  [ "$(f '.billing.quota.week_pct')" = "null" ]
  [ "$(f '.billing.quota.basis')" = "none" ]
}

@test "quota: percentage only against a configured weekly limit" {
  export STATS_WEEKLY_LIMIT=4000000
  OUT="$(agg 5 3100000 | stats_attach_billing)"
  [ "$(f '.billing.quota.week_tokens')" = "3100000" ]
  [ "$(f '.billing.quota.week_pct')" = "77" ]                             # floor(3.1M*100/4M)
  [ "$(f '.billing.quota.basis')" = "config" ]
}

@test "no plan -> subscription is null, API lens still present, no error" {
  OUT="$(agg 100 1000 | stats_attach_billing)"
  [ "$(f '.billing.subscription')" = "null" ]
  [ "$(f '.billing.api.cost')" = "100" ]
}

@test "zero month tokens -> effective rate null (no divide), savings = -fee" {
  export STATS_PLAN=max20
  OUT="$(agg 0 0 | stats_attach_billing)"
  [ "$(f '.billing.subscription.effective_per_mtok')" = "null" ]
  [ "$(f '.billing.subscription.savings')" = "-200" ]
}

@test "an invalid plan name degrades to null subscription, not a crash" {
  export STATS_PLAN=gold
  OUT="$(agg 100 1000 | stats_attach_billing)"
  [ "$(f '.billing.subscription')" = "null" ]
  [ "$(f '.billing.plan_fee_monthly')" = "null" ]
}

@test "billing mode reflects STATS_BILLING (default both)" {
  OUT="$(agg 5 1000 | stats_attach_billing)"
  [ "$(f '.billing.mode')" = "both" ]
  export STATS_BILLING=api
  OUT="$(agg 5 1000 | stats_attach_billing)"
  [ "$(f '.billing.mode')" = "api" ]
}

@test "billing.warnings carries the aggregate's unknown-model warnings" {
  local r='[{"session_id":"s","project":"p","project_path":"/p","totals":{"tokens":1,"input":1,"output":0,"cache_read":0,"cache_write":0,"cost":0},"by_day":{},"by_model":{},"tools":{},"phases":{},"unknown_models":["claude-zz-9"]}]'
  OUT="$(echo "$r" | stats_aggregate | stats_attach_billing)"
  [ "$(f '.billing.warnings|length')" = "1" ]
  [ "$(f '.billing.warnings[0]')" = "unknown model: claude-zz-9 (priced at Sonnet rate)" ]
}

@test "asof from STATS_PRICING_ASOF is surfaced on the billing object" {
  export STATS_PRICING_ASOF=2026-06-15
  OUT="$(agg 5 1000 | stats_attach_billing)"
  unset STATS_PRICING_ASOF
  [ "$(f '.billing.asof')" = "2026-06-15" ]
}
