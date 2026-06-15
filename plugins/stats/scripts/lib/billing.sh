#!/usr/bin/env bash
# billing.sh — attach a `billing` object to the report aggregate, giving both a
# pay-per-token (API-key) lens and a Claude-subscription lens over the SAME
# measured token counts. A jq filter: reads the aggregate on stdin, emits it with
# `.billing` added.
#
# Under a subscription there is no per-token charge — the only real money is the
# flat monthly fee. So the subscription lens reports, on a month-to-date basis:
#   - api_equivalent_value: what this month's tokens WOULD cost on the API
#   - savings:              api_equivalent_value - monthly fee (negative = under-using)
#   - effective_per_mtok:   fee / month-Mtok — the flat fee expressed as a rate
#                           (effective_per_mtok * month_Mtok == fee, exactly)
# and a quota view of measured weekly tokens. Anthropic publishes NO numeric
# Pro/Max ceilings, so a `%` is shown ONLY against a user-configured limit
# (basis:"config"); otherwise basis:"none" and no percentage is invented. The
# 5-hour rolling window is not derivable from the day-bucketed aggregate and is
# left null (deferred).
#
# Inputs via env: STATS_PLAN (pro|max5|max20), STATS_BILLING (api|subscription|
# both), STATS_WEEKLY_LIMIT, STATS_PRICING_ASOF.
set -u

# stats_attach_billing < aggregate.json -> aggregate.json + .billing
stats_attach_billing() {
  local plan="${STATS_PLAN:-}" mode="${STATS_BILLING:-both}"
  local wlim="${STATS_WEEKLY_LIMIT:-}" asof="${STATS_PRICING_ASOF:-}"
  jq -c --arg plan "$plan" --arg mode "$mode" --arg wlim "$wlim" --arg asof "$asof" '
    def fee($p): if $p=="pro" then 20 elif $p=="max5" then 100 elif $p=="max20" then 200 else null end;
    ($wlim | if . == "" then null else (try tonumber catch null) end) as $wl
    | (.windows.month // null) as $m          # null under a --model filter or absent
    | (.windows.week  // null) as $wk
    | (if $plan == "" then null else fee($plan) end) as $f
    | . + { billing: {
        asof: (if $asof == "" then null else $asof end),
        mode: $mode,
        api: { cost: (.totals.cost // 0) },
        plan: (if $plan == "" then null else $plan end),
        plan_fee_monthly: $f,
        subscription: (
          if $f == null or $m == null then null
          else { period: "month-to-date",
                 api_equivalent_value: $m.cost,
                 plan_fee_monthly: $f,
                 savings: ($m.cost - $f),
                 effective_per_mtok: (if $m.tokens > 0 then ($f / ($m.tokens / 1000000)) else null end) }
          end),
        quota: {
          week_tokens: (if $wk == null then null else $wk.tokens end),
          weekly_limit: $wl,
          week_pct: (if ($wl != null and $wl > 0 and $wk != null)
                     then (($wk.tokens * 100) / $wl | floor) else null end),
          basis: (if ($wl != null and $wl > 0) then "config" else "none" end),
          window_5h_tokens: null
        },
        warnings: (.warnings // [])     # pricing warnings (e.g. unknown model), grouped under billing
      } }
  '
}
