#!/usr/bin/env bats
# Tests for the exa-search search.sh env resolution.
# API key is supplied via the EXA_API_KEY environment variable.

# Shared search-test helpers live in tooling/testdata/bats (one copy for
# context7 + exa-search); resolve from this test's dir up to the repo root.
load "${BATS_TEST_DIRNAME}/../../../../../../tooling/testdata/bats/search-helpers"

setup() {
  setup_sandbox exa-search
}

teardown() {
  teardown_sandbox
  unset EXA_API_KEY
}

@test "exa-search: missing EXA_API_KEY exits 1 with clear error" {
  unset EXA_API_KEY
  run "$TOOL_DIR/search.sh" search -q "anything"
  [ "$status" -eq 1 ]
  [[ "$output" == *"EXA_API_KEY unset"* ]]
}

@test "exa-search: EXA_API_KEY env var is sent as x-api-key header" {
  export EXA_API_KEY="test-exa-key-123"
  run "$TOOL_DIR/search.sh" search -q "rust async runtime"
  [ "$status" -eq 0 ]
  grep -q '^x-api-key: test-exa-key-123$' "$CURL_LOG"
}

@test "exa-search: POSTs to https://api.exa.ai/search with query field in body" {
  export EXA_API_KEY="k"
  run "$TOOL_DIR/search.sh" search -q "needle"
  [ "$status" -eq 0 ]
  grep -q '^https://api.exa.ai/search$' "$CURL_LOG"
  # jq pretty-prints the body across lines, so grep for the field independently.
  grep -q '"query":' "$CURL_LOG"
  grep -q '"needle"' "$CURL_LOG"
}

@test "exa-search: help exits 1 with usage banner when no command given" {
  export EXA_API_KEY="k"
  run bash -c '"$1" 2>&1' _ "$TOOL_DIR/search.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Exa Search CLI"* ]]
}

@test "exa-search: search help advertises spec-compliant type enum (no removed 'neural')" {
  export EXA_API_KEY="k"
  run bash -c '"$1" search 2>&1' _ "$TOOL_DIR/search.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"deep-lite"* ]]
  [[ "$output" == *"deep-reasoning"* ]]
  [[ "$output" != *"neural"* ]]
}

@test "exa-search: search help advertises spec-compliant category enum (no removed 'tweet')" {
  export EXA_API_KEY="k"
  run bash -c '"$1" search 2>&1' _ "$TOOL_DIR/search.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"personal site"* ]]
  [[ "$output" == *"financial report"* ]]
  [[ "$output" != *"tweet"* ]]
}

@test "exa-search: --lean flag does not error and still POSTs successfully" {
  export EXA_API_KEY="k"
  # Stub curl returns '{}' which has no .results; lean jq path must tolerate it.
  run "$TOOL_DIR/search.sh" search -q "rust" --lean
  [ "$status" -eq 0 ]
  grep -q '^https://api.exa.ai/search$' "$CURL_LOG"
}
