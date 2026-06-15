#!/usr/bin/env bats
# pricing.sh — per-model cost math. Inputs are sized to 1e6 tokens so the
# expected $ equals the per-Mtok rate exactly (no float fuzz).

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/pricing.sh"
}

# $1=model  $2=usage-json → cost in $
cost() {
  jq -nr --arg m "$1" --argjson u "$2" \
    "$(stats_pricing_jq) rates(\$m) as \$r | msgcost(\$u; \$r)"
}

@test "pricing_id is set" {
  [ -n "$STATS_PRICING_ID" ]
}

@test "opus prices input \$5 + output \$25 per Mtok" {
  run cost "claude-opus-4-8" '{"input_tokens":1000000,"output_tokens":1000000}'
  [ "$status" -eq 0 ]
  [ "$output" = "30" ]
}

@test "haiku prices input \$1 + output \$5 per Mtok" {
  run cost "claude-haiku-4-5-20251001" '{"input_tokens":1000000,"output_tokens":1000000}'
  [ "$output" = "6" ]
}

@test "unknown model falls back to sonnet \$3 input" {
  run cost "some-future-model" '{"input_tokens":1000000}'
  [ "$output" = "3" ]
}

@test "cache_read billed 0.1x input rate (opus)" {
  run cost "claude-opus-4-8" '{"cache_read_input_tokens":1000000}'
  [ "$output" = "0.5" ]
}

@test "cache write 5m TTL billed 1.25x input rate (opus)" {
  run cost "claude-opus-4-8" '{"cache_creation_input_tokens":1000000,"cache_creation":{"ephemeral_5m_input_tokens":1000000,"ephemeral_1h_input_tokens":0}}'
  [ "$output" = "6.25" ]
}

@test "cache write 1h TTL billed 2x input rate (opus)" {
  run cost "claude-opus-4-8" '{"cache_creation_input_tokens":1000000,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":1000000}}'
  [ "$output" = "10" ]
}

@test "cache write falls back to 1.25x when no ephemeral split present" {
  run cost "claude-opus-4-8" '{"cache_creation_input_tokens":1000000}'
  [ "$output" = "6.25" ]
}

# --- Fable / Mythos: the previously-missing tier ($10/$50, 2x Opus) ---

@test "fable prices input \$10 + output \$50 per Mtok" {
  run cost "claude-fable-5" '{"input_tokens":1000000,"output_tokens":1000000}'
  [ "$status" -eq 0 ]
  [ "$output" = "60" ]
}

@test "mythos is priced as fable \$10 input" {
  run cost "claude-mythos-5" '{"input_tokens":1000000}'
  [ "$output" = "10" ]
}

@test "fable cache_read billed 0.1x = \$1 per Mtok" {
  run cost "claude-fable-5" '{"cache_read_input_tokens":1000000}'
  [ "$output" = "1" ]
}

@test "fable cache write 5m 1.25x = \$12.50 per Mtok" {
  run cost "claude-fable-5" '{"cache_creation":{"ephemeral_5m_input_tokens":1000000,"ephemeral_1h_input_tokens":0}}'
  [ "$output" = "12.5" ]
}

@test "fable cache write 1h 2x = \$20 per Mtok" {
  run cost "claude-fable-5" '{"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":1000000}}'
  [ "$output" = "20" ]
}

# --- Mixed TTL in one message: 5m and 1h writes both counted ---

@test "mixed 5m+1h in one message sums both writes (opus 6.25 + 10)" {
  run cost "claude-opus-4-8" '{"cache_creation":{"ephemeral_5m_input_tokens":1000000,"ephemeral_1h_input_tokens":1000000}}'
  [ "$output" = "16.25" ]
}

# --- Unknown-model flag: priced at Sonnet, but tagged so callers can warn ---

@test "unknown model is flagged unknown:true on the rate struct" {
  run jq -nr --arg m "some-future-model" "$(stats_pricing_jq) rates(\$m) | .unknown // false"
  [ "$status" -eq 0 ]
  [ "$output" = "true" ]
}

@test "known models are not flagged unknown" {
  run jq -nr --arg m "claude-opus-4-8" "$(stats_pricing_jq) rates(\$m) | .unknown // false"
  [ "$output" = "false" ]
}

@test "sonnet is explicitly matched, not via the unknown branch" {
  run jq -nr --arg m "claude-sonnet-4-6" "$(stats_pricing_jq) rates(\$m) | [.i,.o,(.unknown // false)]|map(tostring)|join(\",\")"
  [ "$output" = "3,15,false" ]
}
