#!/usr/bin/env bash
# usage.sh — compute one session's usage rollup from its transcript file set.
#
# benchmarks' own copy of the token/cost rollup math (originally written for the
# now-removed `stats` plugin's report; kept here as the single source of truth
# for benchmarks' live-tier token accounting — sourced not copied, so
# cases/cavecrew/run.sh and cases/whole-session/run.sh share one implementation).
# Given a session's main transcript plus its subagent transcripts, it: parses
# each line tolerantly (a malformed/truncated line is skipped, never fatal),
# keeps only assistant messages, dedups by message.id (Claude Code re-writes the
# same id as tokens stream), prices each message at its model rate
# (pricing.sh), buckets by LOCAL day, and emits a single rollup object: totals +
# by_day + by_model + tool-mix + phase counts.
#
# `tokens` is the rate-limit-pacing total (input + output + cache_write);
# cache_read is tracked separately, not folded in — it is ~98% of volume but
# billed ~0.1x. Project identity (.cwd) is surfaced raw, unresolved.
set -u

# stats_usage_rollup <transcript> [subagent-transcript ...] -> rollup JSON on stdout.
# Emits an all-zero rollup (cwd null) when no assistant messages are present.
stats_usage_rollup() {
  local lib; lib="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  # shellcheck source=/dev/null
  source "$lib/pricing.sh"
  # Append a newline after each file so the last line of one transcript never
  # fuses with the first line of the next (a missing trailing newline would
  # otherwise make both lines unparseable). Empty lines are dropped by fromjson?.
  { for _f in "$@"; do cat -- "$_f" 2>/dev/null; printf '\n'; done; } | jq -Rs "$(stats_pricing_jq)"'
    def bucket(ts): (ts | sub("\\.[0-9]+Z$"; "Z") | fromdateiso8601 | strflocaltime("%Y-%m-%d"));
    def sums:
      { tokens:      (map(.tokens)      | add // 0),
        input:       (map(.in)          | add // 0),
        output:      (map(.out)         | add // 0),
        cache_read:  (map(.cache_read)  | add // 0),
        cache_write: (map(.cache_write) | add // 0),
        cost:        (map(.cost)        | add // 0) };
    ( split("\n") | map(fromjson?) )
    | [ .[]
        | select(type == "object" and .type == "assistant")
        | select(.message.id != null and .timestamp != null)
        | (.message.model // "") as $m
        | (.message.usage // {}) as $u
        | (rates($m)) as $r
        | { id:          .message.id,
            day:         (try bucket(.timestamp) catch null),
            model:       $m,
            skill:       (.attributionSkill // null),
            tools:       [ (.message.content // [])[]? | select(.type == "tool_use") | .name ],
            tokens:      (($u.input_tokens // 0) + ($u.output_tokens // 0) + ($u.cache_creation_input_tokens // 0)),
            in:          ($u.input_tokens // 0),
            out:         ($u.output_tokens // 0),
            cache_read:  ($u.cache_read_input_tokens // 0),
            cache_write: ($u.cache_creation_input_tokens // 0),
            cost:        msgcost($u; $r),
            unknown:     ($r.unknown // false),
            cwd:         (.cwd // null) } ]
    | map(select(.day != null))
    | group_by(.id) | map(.[-1])                        # dedup streaming dups: keep the FINAL frame (full content/tool_use; usage is identical across frames)
    | . as $msgs
    | { messages: ($msgs | length),
        cwd:      ( [ $msgs[].cwd | select(. != null) ][0] // null ),
        totals:   ( $msgs | sums ),
        by_day:   ( $msgs | group_by(.day)   | map({ key: .[0].day,   value: (. | sums) }) | from_entries ),
        by_model: ( $msgs | group_by(.model) | map({ key: .[0].model, value: (. | sums) }) | from_entries ),
        tools:    ( [ $msgs[].tools[] ]      | group_by(.) | map({ key: .[0], value: length }) | from_entries ),
        phases:   ( [ $msgs[].skill | select(. != null) | sub("^toolu:"; "") ]
                    | group_by(.) | map({ key: .[0], value: length }) | from_entries ),
        unknown_models: ( [ $msgs[] | select(.unknown) | .model ] | unique ) }
  '
}
