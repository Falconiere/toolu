#!/usr/bin/env bats
# render.sh — humanizers, the glyph dashboard, --json. The aggregate is produced
# from crafted rollups (reusing aggregate.sh) so the digest reflects real shapes.

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/aggregate.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/render.sh"
  TODAY="$(date +%Y-%m-%d)"
  full() { echo "{\"tokens\":$1,\"input\":$1,\"output\":0,\"cache_read\":0,\"cache_write\":0,\"cost\":$2}"; }
  ROLLUPS=$(cat <<EOF
[
 {"session_id":"sess-aaaa1111","project":"toolu.sh","project_path":"/Volumes/Projects/toolu.sh",
  "totals":$(full 1500000 4.20),
  "by_day":{"$TODAY":$(full 1500000 4.20)},
  "by_model":{"claude-opus-4-8":$(full 1500000 4.20)},
  "tools":{"Read":5,"Bash":2},"phases":{"execution":3}}
]
EOF
)
  AGG="$(echo "$ROLLUPS" | stats_aggregate)"
}

rnd() { echo "$AGG" | stats_render; }

@test "format_tokens humanizes M / k / raw" {
  [ "$(stats_format_tokens 13779513)" = "13.7M" ]
  [ "$(stats_format_tokens 45000)" = "45k" ]
  [ "$(stats_format_tokens 999)" = "999" ]
  [ "$(stats_format_tokens notanumber)" = "0" ]
}

@test "--json emits the raw aggregate, valid and complete" {
  export STATS_OUTPUT=json; run rnd; unset STATS_OUTPUT
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.totals.tokens == 1500000' >/dev/null
  echo "$output" | jq -e '.by_project[0].project_path == "/Volumes/Projects/toolu.sh"' >/dev/null
  echo "$output" | jq -e '.daily | length == 14' >/dev/null
}

@test "dashboard shows boxed header, gauge, trend, sections, activity" {
  run rnd
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Claude Code Usage"
  echo "$output" | grep -q "1.5M tokens"
  echo "$output" | grep -q "cache hit 0%"
  echo "$output" | grep -q "Trend 14d"
  echo "$output" | grep -q "today 1.5M"
  echo "$output" | grep -q "Projects"
  echo "$output" | grep -q "toolu.sh"
  echo "$output" | grep -q "Models"
  echo "$output" | grep -q "claude-opus-4-8"
  echo "$output" | grep -q "Top sessions"
  echo "$output" | grep -q "Read:5"
  echo "$output" | grep -q "Bash:2"
  echo "$output" | grep -q "phases: execution:3"
  echo "$output" | grep -q "gate:"
}

@test "dashboard draws bar glyphs and a box rule" {
  run rnd
  echo "$output" | grep -q "█"
  echo "$output" | grep -q "┌"
  echo "$output" | grep -q "└"
}

@test "model-filtered aggregate renders trend/windows as n/a" {
  export STATS_MODEL=opus; AGG="$(echo "$ROLLUPS" | stats_aggregate)"; unset STATS_MODEL
  run rnd
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "n/a under --model filter"
}

@test "cost renders with a dot under a comma-decimal locale" {
  # Find an installed comma-decimal locale; skip on minimal images that lack one.
  local loc="" c
  for c in de_DE.UTF-8 fr_FR.UTF-8 nl_NL.UTF-8 pt_BR.UTF-8 es_ES.UTF-8; do
    locale -a 2>/dev/null | grep -qiF "$c" || continue
    # Probe with an INTEGER (parses in any locale); a comma in the output marks a
    # comma-decimal locale. (A dot-input probe would itself error under one.)
    [ "$(LC_ALL=$c printf '%.1f' 1 2>/dev/null)" = "1,0" ] && { loc="$c"; break; }
  done
  [ -n "$loc" ] || skip "no comma-decimal locale installed"
  export LC_ALL="$loc"; run rnd; unset LC_ALL
  [ "$status" -eq 0 ]
  echo "$output" | grep -q '\$4\.20'      # dot + correct value, not $4,00 or a printf error
}

@test "empty usage degrades to a friendly notice" {
  AGG="$(echo '[]' | stats_aggregate)"
  run rnd
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no usage recorded"
}

@test "dashboard surfaces an unknown-model pricing warning" {
  local r='[{"session_id":"s","project":"p","project_path":"/p","totals":{"tokens":10,"input":10,"output":0,"cache_read":0,"cache_write":0,"cost":0.01},"by_day":{"'"$TODAY"'":{"tokens":10,"input":10,"output":0,"cache_read":0,"cache_write":0,"cost":0.01}},"by_model":{"claude-zz-9":{"tokens":10,"input":10,"output":0,"cache_read":0,"cache_write":0,"cost":0.01}},"tools":{},"phases":{},"unknown_models":["claude-zz-9"]}]'
  AGG="$(echo "$r" | stats_aggregate)"
  run rnd
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "pricing warnings"
  echo "$output" | grep -q "claude-zz-9"
}
