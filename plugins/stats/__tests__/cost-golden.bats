#!/usr/bin/env bats
# Golden cost: a pinned REAL fixture session must price to an exact dollar amount,
# and that one number must render identically across --json (raw float), the
# terminal dashboard, and the HTML report ($X.XX). Guards against a formula
# change silently shifting cost and against the three surfaces diverging.
#
# multimodel.jsonl: opus + haiku messages, deduped by id. Expected total:
#   opus  0.17824800  +  haiku  0.01269875  =  0.19094675  ->  $0.19

setup() {
  export TZ=UTC
  source "${BATS_TEST_DIRNAME}/../scripts/lib/usage.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/aggregate.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/render.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/render-html.sh"
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  export STATS_NO_OPEN=1
  F="${BATS_TEST_DIRNAME}/fixtures"
  EXPECT_COST="0.19094675"
  EXPECT_MONEY="0.19"
  roll="$(stats_usage_rollup "$F/multimodel.jsonl" \
          | jq -c '. + {session_id:"golden",project:"toolu.sh",project_path:"/Volumes/Projects/toolu.sh"}')"
  AGG="$(printf '[%s]' "$roll" | stats_aggregate)"
  REPORT="$CLAUDE_CONFIG_DIR/stats/report.html"
}

@test "the pinned fixture prices to the exact expected total cost" {
  [ "$(printf '%s' "$AGG" | jq -r '.totals.cost')" = "$EXPECT_COST" ]
}

@test "--json carries the exact cost as a raw float" {
  local jcost
  jcost="$(printf '%s' "$AGG" | STATS_OUTPUT=json stats_render | jq -r '.totals.cost')"
  [ "$jcost" = "$EXPECT_COST" ]
}

@test "terminal dashboard shows the same cost as \$0.19" {
  printf '%s' "$AGG" | STATS_OUTPUT=text stats_render | grep -qF "\$$EXPECT_MONEY"
}

@test "HTML report shows the same cost as \$0.19" {
  printf '%s' "$AGG" | stats_render_html >/dev/null
  [ -f "$REPORT" ]
  grep -qF "$EXPECT_MONEY" "$REPORT"
}
