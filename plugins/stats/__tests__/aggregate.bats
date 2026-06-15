#!/usr/bin/env bats
# aggregate.sh — rollups + filters → aggregate object. Inputs are crafted
# rollups (today's date injected) so window math is deterministic.

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/aggregate.sh"
  TODAY="$(date +%Y-%m-%d)"
  # s1 alpha @/a/alpha (opus, 150: 100 today + 50 in 2020), s2 beta @/b/beta
  # (haiku, 80 today), s3 alpha @/c/alpha — SAME basename, different path.
  full() { echo "{\"tokens\":$1,\"input\":$1,\"output\":0,\"cache_read\":0,\"cache_write\":0,\"cost\":$2}"; }
  ROLLUPS=$(cat <<EOF
[
 {"session_id":"s1","project":"alpha","project_path":"/a/alpha",
  "totals":$(full 150 0.15),
  "by_day":{"$TODAY":$(full 100 0.10),"2020-01-01":$(full 50 0.05)},
  "by_model":{"claude-opus-4-8":$(full 150 0.15)},
  "tools":{"Read":2,"Grep":1},"phases":{"spec":1}},
 {"session_id":"s2","project":"beta","project_path":"/b/beta",
  "totals":$(full 80 0.08),
  "by_day":{"$TODAY":$(full 80 0.08)},
  "by_model":{"claude-haiku-4-5-20251001":$(full 80 0.08)},
  "tools":{"Bash":3},"phases":{"plan":2}},
 {"session_id":"s3","project":"alpha","project_path":"/c/alpha",
  "totals":$(full 40 0.04),
  "by_day":{"$TODAY":$(full 40 0.04)},
  "by_model":{"claude-opus-4-8":$(full 40 0.04)},
  "tools":{},"phases":{}}
]
EOF
)
}

agg() { echo "$ROLLUPS" | stats_aggregate; }
j() { echo "$output" | jq -r "$1"; }

@test "totals sum across sessions" {
  run agg
  [ "$status" -eq 0 ]
  [ "$(j '.totals.tokens')" = "270" ]
  [ "$(j '.totals.sessions')" = "3" ]
  [ "$(j '.totals.cache_hit_pct')" = "0" ]
}

@test "windows: today excludes the 2020 day; all includes it" {
  run agg
  [ "$(j '.windows.today.tokens')" = "220" ]   # 100+80+40
  [ "$(j '.windows.week.tokens')" = "220" ]     # 2020 day not in this week
  [ "$(j '.windows.all.tokens')" = "270" ]      # +50
}

@test "daily: 14-day series, oldest-first, today last with summed tokens" {
  run agg
  [ "$(j '.daily|length')" = "14" ]
  [ "$(j '.daily[-1].date')" = "$TODAY" ]
  [ "$(j '.daily[-1].tokens')" = "220" ]          # 100+80+40 today
  [ "$(j '[.daily[].tokens]|add')" = "220" ]       # 2020 day falls outside the 14-day window
}

@test "by_project groups on project_path, not basename (alpha stays split)" {
  run agg
  [ "$(j '.by_project|length')" = "3" ]
  [ "$(j '[.by_project[]|select(.project=="alpha")]|length')" = "2" ]
  [ "$(j '.by_project[0].project_path')" = "/a/alpha" ]   # highest tokens first
}

@test "by_model merges across sessions" {
  run agg
  [ "$(j '.by_model[0].model')" = "claude-opus-4-8" ]
  [ "$(j '.by_model[0].tokens')" = "190" ]      # 150 + 40
  [ "$(j '[.by_model[]|select(.model|test("haiku"))][0].tokens')" = "80" ]
}

@test "top_sessions ranked by tokens" {
  run agg
  [ "$(j '.top_sessions[0].session_id')" = "s1" ]
  [ "$(j '.top_sessions[-1].session_id')" = "s3" ]
}

@test "activity merges tool-mix and phases" {
  run agg
  [ "$(j '.activity.tools.Read')" = "2" ]
  [ "$(j '.activity.tools.Bash')" = "3" ]
  [ "$(j '.activity.phases.plan')" = "2" ]
}

@test "--model narrows totals to that model; windows go null" {
  export STATS_MODEL=opus; run agg; unset STATS_MODEL
  [ "$status" -eq 0 ]
  [ "$(j '.totals.tokens')" = "190" ]           # opus only: 150 + 40
  [ "$(j '.windows')" = "null" ]
  [ "$(j '.daily')" = "null" ]
  [ "$(j '.by_model|length')" = "1" ]
}

@test "--project filters on label or path" {
  export STATS_PROJECT=beta; run agg; unset STATS_PROJECT
  [ "$(j '.totals.tokens')" = "80" ]
  [ "$(j '.totals.sessions')" = "1" ]
}

@test "--session filters to one session" {
  export STATS_SESSION=s3; run agg; unset STATS_SESSION
  [ "$(j '.totals.tokens')" = "40" ]
  [ "$(j '.totals.sessions')" = "1" ]
}

@test "gate/comemory snapshots degrade to unknown/0 when absent" {
  cd "$BATS_TEST_TMPDIR"      # no git root, no .claude/tmp here
  run agg
  [ "$(j '.activity.gate')" = "unknown" ]
  [ "$(j '.activity.comemory')" = "0" ]
}

@test "empty rollups yield zeroed totals, not an error" {
  ROLLUPS='[]' run agg
  [ "$status" -eq 0 ]
  [ "$(j '.totals.tokens')" = "0" ]
  [ "$(j '.totals.sessions')" = "0" ]
  [ "$(j '.by_project|length')" = "0" ]
  [ "$(j '.warnings|length')" = "0" ]
}

@test "surfaces unknown-model warnings, deduped across sessions" {
  ROLLUPS='[{"session_id":"x","project":"p","project_path":"/p","totals":{"tokens":1,"input":1,"output":0,"cache_read":0,"cache_write":0,"cost":0},"by_day":{},"by_model":{},"tools":{},"phases":{},"unknown_models":["claude-zz-9"]},{"session_id":"y","project":"p","project_path":"/p","totals":{"tokens":1,"input":1,"output":0,"cache_read":0,"cache_write":0,"cost":0},"by_day":{},"by_model":{},"tools":{},"phases":{},"unknown_models":["claude-zz-9"]}]' run agg
  [ "$status" -eq 0 ]
  [ "$(j '.warnings|length')" = "1" ]
  [ "$(j '.warnings[0]')" = "unknown model: claude-zz-9 (priced at Sonnet rate)" ]
}

@test "no warnings when all models are known" {
  run agg
  [ "$(j '.warnings|length')" = "0" ]
}
