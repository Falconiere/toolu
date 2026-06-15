#!/usr/bin/env bash
# aggregate.sh — reduce the per-session rollups (from scan_all) into the report
# aggregate: economic totals, time windows, per-project / per-model / per-session
# breakdowns, and an activity block.
#
# Reads a JSON array of enriched rollups on stdin; filters and options come from
# env vars (STATS_PROJECT / STATS_MODEL / STATS_SESSION / STATS_WINDOW /
# STATS_LIMIT). by_project is grouped on project_path (full path) so two repos
# sharing a basename never merge. --model narrows the per-model totals exactly
# (from by_model); time windows can't be sliced by model and are reported null
# under a model filter. gate/comemory are read-only snapshots of current state.
set -u

# Current quality-gate status at the git root of $PWD (unknown when absent).
stats_gate_status() {
  local root file
  root="$(git -C "$PWD" --no-optional-locks rev-parse --show-toplevel 2>/dev/null)"
  file="${root:-$PWD}/.claude/tmp/quality-gate-status.json"
  [ -f "$file" ] && jq -r '.status // "unknown"' "$file" 2>/dev/null || echo "unknown"
}

# comemory memory count for the current repo (0 when absent). Key derivation
# matches the comemory status publisher: main-repo basename via git-common-dir.
stats_comemory_count() {
  local cfg ck file
  cfg="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  ck="$(git -C "$PWD" --no-optional-locks rev-parse --git-common-dir 2>/dev/null)" || { echo 0; return; }
  case "$ck" in /*) : ;; *) ck="$(cd "$PWD" 2>/dev/null && cd "$ck" 2>/dev/null && pwd)" ;; esac
  [ -n "$ck" ] || { echo 0; return; }
  file="$cfg/comemory-status/$(basename "$(dirname "$ck")").json"
  [ -f "$file" ] && jq -r '.count // 0' "$file" 2>/dev/null || echo 0
}

# stats_aggregate < rollups.json -> aggregate object (compact)
stats_aggregate() {
  local today week now limit model proj session window gate comem
  today="$(date +%Y-%m-%d)"; week="$(date +%G-W%V)"; now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  limit="${STATS_LIMIT:-10}"; model="${STATS_MODEL:-}"; proj="${STATS_PROJECT:-}"
  session="${STATS_SESSION:-}"; window="${STATS_WINDOW:-all}"
  gate="$(stats_gate_status)"; comem="$(stats_comemory_count)"
  jq -c \
    --arg today "$today" --arg week "$week" --arg now "$now" --argjson limit "$limit" \
    --arg model "$model" --arg proj "$proj" --arg session "$session" --arg window "$window" \
    --arg gate "$gate" --argjson comem "$comem" '
    def weekof(d): (d + "T12:00:00Z" | fromdateiso8601 | strftime("%G-W%V"));
    def zero: {tokens:0,input:0,output:0,cache_read:0,cache_write:0,cost:0};
    def addt(b): {tokens:(.tokens+b.tokens), input:(.input+b.input), output:(.output+b.output),
                  cache_read:(.cache_read+b.cache_read), cache_write:(.cache_write+b.cache_write),
                  cost:(.cost+b.cost)};
    def sumt(a): reduce a[] as $x (zero; addt($x));

    map(select( ($session == "" or .session_id == $session)
            and ($proj == "" or .project == $proj or .project_path == $proj) ))
    | ( if $model != "" then
          map( . as $r
            | [ ($r.by_model // {}) | to_entries[] | select(.key | contains($model)) ] as $mm
            | select(($mm | length) > 0)
            | $r + { totals: sumt([ $mm[].value ]), by_model: ($mm | from_entries), model_filtered: true } )
        else . end ) as $sessions
    | ( [ $sessions[] | (.by_day // {}) | to_entries[] ] ) as $days
    | { generated_at: $now,
        window: $window,
        totals: ( sumt([ $sessions[].totals ]) as $t
                  | $t + { cache_hit_pct: (if ($t.cache_read + $t.input) > 0
                                           then ( ($t.cache_read * 100) / ($t.cache_read + $t.input) | floor )
                                           else 0 end),
                           sessions: ($sessions | length) } ),
        windows: ( if $model != "" then null else
                     { today: ( sumt([ $days[] | select(.key == $today) | .value ]) | {tokens, cost} ),
                       week:  ( sumt([ $days[] | select(weekof(.key) == $week) | .value ]) | {tokens, cost} ),
                       all:   ( sumt([ $days[].value ]) | {tokens, cost} ) } end ),
        daily: ( if $model != "" then null else
                   ( [ $days[] | {key, t: .value.tokens, c: .value.cost} ] | group_by(.key)
                     | map({ key: .[0].key, value: { tokens: (map(.t) | add), cost: (map(.c) | add) } }) | from_entries ) as $dl
                   | [ range(0;14) | (($today + "T12:00:00Z" | fromdateiso8601) - (. * 86400) | strftime("%Y-%m-%d")) ]
                   | reverse
                   | map(. as $d | { date: $d, tokens: ($dl[$d].tokens // 0), cost: ($dl[$d].cost // 0) }) end ),
        by_project: ( $sessions | group_by(.project_path)
                      | map({ project: .[0].project, project_path: .[0].project_path,
                              sessions: length,
                              tokens: (sumt([ .[].totals ]).tokens),
                              cost:   (sumt([ .[].totals ]).cost) })
                      | sort_by(.tokens) | reverse ),
        by_model: ( $sessions | [ .[] | (.by_model // {}) | to_entries[] ] | group_by(.key)
                    | map({ model: .[0].key, tokens: (map(.value.tokens) | add),
                            cost: (map(.value.cost) | add) })
                    | sort_by(.tokens) | reverse ),
        top_sessions: ( $sessions | map({ session_id, project, tokens: .totals.tokens, cost: .totals.cost })
                        | sort_by(.tokens) | reverse | .[0:$limit] ),
        activity: { tools:  ( $sessions | [ .[] | (.tools // {})  | to_entries[] ] | group_by(.key)
                              | map({ key: .[0].key, value: (map(.value) | add) }) | from_entries ),
                    phases: ( $sessions | [ .[] | (.phases // {}) | to_entries[] ] | group_by(.key)
                              | map({ key: .[0].key, value: (map(.value) | add) }) | from_entries ),
                    gate: $gate, comemory: $comem } }
  '
}
