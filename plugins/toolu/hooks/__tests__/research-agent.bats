#!/usr/bin/env bats
# Tests for agents/research-agent.md and the exa fallback signal it relies on.
# Asserts the REAL agent file's frontmatter/flags, and runs the REAL exa
# search.sh keyless to confirm it exits 1 (the documented try-then-fallback
# trigger — spec acceptance #4). No mocks.

AGENT="${BATS_TEST_DIRNAME}/../../agents/research-agent.md"
EXA="${BATS_TEST_DIRNAME}/../../../exa-search/skills/exa-search/scripts/search.sh"

@test "research-agent: file exists" {
  [ -f "$AGENT" ]
}

@test "research-agent: frontmatter declares name, sonnet tier, and tool set" {
  grep -q '^name: research-agent$' "$AGENT"
  grep -q '^model: sonnet$' "$AGENT"
  grep -q '^tools: Read, Bash, WebSearch, WebFetch, Grep, Glob$' "$AGENT"
}

@test "research-agent: description names both boundaries (deep-explore, deep-research)" {
  grep -q 'deep-explore' "$AGENT"
  grep -q 'deep-research' "$AGENT"
}

@test "research-agent: body documents both routing CLIs" {
  grep -q 'context7' "$AGENT"
  grep -q 'exa-search' "$AGENT"
}

@test "research-agent: body documents lean token flags" {
  grep -q -- '--lean' "$AGENT"
  grep -q -- '--fast' "$AGENT"
}

@test "research-agent: body documents the output contract" {
  grep -q 'Sources:' "$AGENT"
  grep -q 'Tools used:' "$AGENT"
}

@test "research-agent: body documents the no-search staleness floor" {
  grep -qi 'stale' "$AGENT"
}

@test "research-agent: real exa search.sh exits 1 with no API key (fallback signal)" {
  # Skip only if the script's own hard deps are missing — then exit 1 would be
  # for the wrong reason and the test would not assert the key path.
  command -v jq >/dev/null 2>&1   || skip "jq not installed"
  command -v curl >/dev/null 2>&1 || skip "curl not installed"
  run env -u EXA_API_KEY bash "$EXA" search -q "anything"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q 'EXA_API_KEY unset'
}
