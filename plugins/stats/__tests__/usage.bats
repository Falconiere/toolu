#!/usr/bin/env bats
# usage.sh — transcript file-set → session rollup. Expected numbers are computed
# from the real fixtures; TZ=UTC pins the day boundary (matches statusline).

setup() {
  export TZ=UTC
  source "${BATS_TEST_DIRNAME}/../scripts/lib/usage.sh"
  F="${BATS_TEST_DIRNAME}/fixtures"
}

# jq query against the last `run` output
j() { echo "$output" | jq -r "$1"; }

@test "dedups streaming duplicates by message.id" {
  run stats_usage_rollup "$F/dup.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.messages')" = "25" ]        # 50 raw assistant lines, 25 distinct ids
  [ "$(j '.totals.tokens')" = "148517" ]
}

@test "rolls subagent transcripts into the session total" {
  run stats_usage_rollup "$F/sub-session.jsonl" "$F"/sub-session/subagents/agent-*.jsonl
  [ "$status" -eq 0 ]
  [ "$(j '.messages')" = "102" ]
  # 274741, not the 268219 statusline records: streamed output_tokens grow across
  # frames, so the FINAL frame is complete — first-frame dedup undercounts output.
  [ "$(j '.totals.tokens')" = "274741" ]
  # per-model split sums back to the total (subagents are haiku, main is opus)
  [ "$(j '.by_model|keys|length')" = "2" ]
  [ "$(j '.by_model|map(.tokens)|add')" = "274741" ]
}

@test "tokens exclude cache_read; cache_read tracked separately" {
  run stats_usage_rollup "$F/multimodel.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.totals.tokens == (.totals.input + .totals.output + .totals.cache_write)')" = "true" ]
  [ "$(j '.totals.cache_read > 0')" = "true" ]
}

@test "prices per model; by_model costs sum to the total cost" {
  run stats_usage_rollup "$F/sub-session.jsonl" "$F"/sub-session/subagents/agent-*.jsonl
  [ "$status" -eq 0 ]
  [ "$(j '.totals.cost > 0')" = "true" ]
  [ "$(j '((.by_model|map(.cost)|add)*1000000|round) == (.totals.cost*1000000|round)')" = "true" ]
}

@test "buckets by local day; a UTC-midnight straddle splits into two days" {
  run stats_usage_rollup "$F/straddle.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.by_day|keys|join(",")')" = "2026-06-07,2026-06-08" ]
}

@test "surfaces .cwd for project attribution" {
  run stats_usage_rollup "$F/multimodel.jsonl"
  [ "$(j '.cwd')" = "/Volumes/Projects/toolu.sh" ]
}

@test "reports null cwd when transcript carries none (slug fallback handled in scan)" {
  run stats_usage_rollup "$F/nocwd.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.cwd')" = "null" ]
}

@test "counts tool-mix from finalized tool_use blocks" {
  run stats_usage_rollup "$F/multimodel.jsonl"
  [ "$(j '.tools.Bash')" = "1" ]
}

@test "counts attributed skills (toolu phases shown without the prefix)" {
  run stats_usage_rollup "$F/multimodel.jsonl"
  [ "$(j '.phases["statusline:setup"]')" = "2" ]
}

@test "skips a malformed/truncated line instead of aborting" {
  run stats_usage_rollup "$F/malformed.jsonl"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e . >/dev/null      # valid JSON produced
  [ "$(j '.messages >= 1')" = "true" ]
}

@test "empty input yields an all-zero rollup, not an error" {
  run stats_usage_rollup /dev/null
  [ "$status" -eq 0 ]
  [ "$(j '.messages')" = "0" ]
  [ "$(j '.totals.tokens')" = "0" ]
  [ "$(j '.cwd')" = "null" ]
  [ "$(j '.unknown_models|length')" = "0" ]
}

@test "flags an unknown model in unknown_models so the caller can warn" {
  printf '%s\n' '{"type":"assistant","timestamp":"2026-06-14T12:00:00Z","message":{"id":"m1","model":"claude-zztop-9","usage":{"input_tokens":10}}}' > "$BATS_TEST_TMPDIR/u.jsonl"
  run stats_usage_rollup "$BATS_TEST_TMPDIR/u.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.unknown_models|join(",")')" = "claude-zztop-9" ]
}

@test "known models leave unknown_models empty" {
  run stats_usage_rollup "$F/multimodel.jsonl"
  [ "$status" -eq 0 ]
  [ "$(j '.unknown_models|length')" = "0" ]
}
