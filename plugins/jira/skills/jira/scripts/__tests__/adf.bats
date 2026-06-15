#!/usr/bin/env bats
# Tests for lib/adf.sh — version-gated text->body rendering.

load helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

@test "adf: v3 wraps text in a minimal ADF doc" {
  run bash -c 'source "$1/lib/adf.sh"; _JIRA_VER=3 jira_text_to_adf "hello world"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.type=="doc" and .version==1 and .content[0].content[0].text=="hello world"' >/dev/null
}

@test "adf: v2 passes text through as a JSON string" {
  run bash -c 'source "$1/lib/adf.sh"; _JIRA_VER=2 jira_text_to_adf "hello world"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r 'type')" = "string" ]
  [ "$(echo "$output" | jq -r '.')" = "hello world" ]
}

@test "adf: default (no _JIRA_VER) renders v3 ADF" {
  run bash -c 'source "$1/lib/adf.sh"; jira_text_to_adf "x"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.type=="doc"' >/dev/null
}

@test "adf: quotes and backslashes are JSON-escaped" {
  run bash -c 'source "$1/lib/adf.sh"; _JIRA_VER=3 jira_text_to_adf "a \" b \\ c"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.content[0].content[0].text=="a \" b \\ c"' >/dev/null
}
