#!/usr/bin/env bats
# Tests for lib/sprint.sh — Agile sprint family: list/get/issues (GET), create
# (originBoardId+name), move (issues array), start/complete (state mutations).
# Agile endpoints always use /rest/agile/1.0. Asserts on the request the script
# builds via the curl stub.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/sprint.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "sprint list hits the board sprint endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint list 5' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/board/5/sprint' "$CURL_LOG"
}

@test "sprint get hits the sprint endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint get 9' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint/9' "$CURL_LOG"
}

@test "sprint issues hits the sprint issue endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint issues 9' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint/9/issue' "$CURL_LOG"
}

@test "sprint create POSTs originBoardId and name to the sprint endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint create 5 -n "Sprint 1"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint' "$CURL_LOG"
  grep -q '"originBoardId":5' "$CURL_LOG"
  grep -q '"name":"Sprint 1"' "$CURL_LOG"
}

@test "sprint create missing name exits 1" {
  run bash -c "$S"'; jira_require_env && jira_sprint create 5' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira sprint create"* ]]
}

@test "sprint move POSTs the issue keys array" {
  run bash -c "$S"'; jira_require_env && jira_sprint move 9 ABC-1 ABC-2' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint/9/issue' "$CURL_LOG"
  grep -q '"issues":\["ABC-1","ABC-2"\]' "$CURL_LOG"
}

@test "sprint move missing keys exits 1" {
  run bash -c "$S"'; jira_require_env && jira_sprint move 9' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira sprint move"* ]]
}

@test "sprint start POSTs active state to the sprint endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint start 9' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint/9' "$CURL_LOG"
  grep -q '{"state":"active"}' "$CURL_LOG"
}

@test "sprint complete POSTs closed state to the sprint endpoint" {
  run bash -c "$S"'; jira_require_env && jira_sprint complete 9' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/sprint/9' "$CURL_LOG"
  grep -q '{"state":"closed"}' "$CURL_LOG"
  run grep -q '{"state":"active"}' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "sprint unknown action exits 1" {
  run bash -c "$S"'; jira_require_env && jira_sprint bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira sprint <list|get|issues|create|move|start|complete>"* ]]
}
