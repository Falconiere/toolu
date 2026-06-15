#!/usr/bin/env bats
# Tests for lib/board.sh — Agile board family; the curl stub records argv to
# $CURL_LOG so we assert the exact /rest/agile/1.0 URLs and query params.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/board.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "board list hits /rest/agile/1.0/board" {
  run bash -c "$S"'; jira_require_env && jira_board list' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/board' "$CURL_LOG"
  run grep -q 'projectKeyOrId' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "board list -p ABC includes projectKeyOrId=ABC" {
  run bash -c "$S"'; jira_require_env && jira_board list -p ABC' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/board?projectKeyOrId=ABC' "$CURL_LOG"
}

@test "board get 5 hits /rest/agile/1.0/board/5" {
  run bash -c "$S"'; jira_require_env && jira_board get 5' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/board/5' "$CURL_LOG"
}

@test "board issues 5 hits /rest/agile/1.0/board/5/issue" {
  run bash -c "$S"'; jira_require_env && jira_board issues 5' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/agile/1.0/board/5/issue' "$CURL_LOG"
  run grep -q 'jql=' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "board issues 5 -q URL-encodes the jql in the query string" {
  run bash -c "$S"'; jira_require_env && jira_board issues 5 -q "status=Done"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q 'jql=status%3DDone' "$CURL_LOG"
  grep -q '/rest/agile/1.0/board/5/issue?' "$CURL_LOG"
}

@test "board issues 5 -q URL-encodes spaces in a multi-word JQL" {
  run bash -c "$S"'; jira_require_env && jira_board issues 5 -q "status = Done"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q 'jql=status%20%3D%20Done' "$CURL_LOG"
}

@test "board issues 5 -n includes maxResults in the query string" {
  run bash -c "$S"'; jira_require_env && jira_board issues 5 -n 10' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q 'maxResults=10' "$CURL_LOG"
}

@test "board unknown action exits 1 with usage" {
  run bash -c "$S"'; jira_require_env && jira_board bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira board"* ]]
}

@test "board get with no id exits 1 with usage" {
  run bash -c "$S"'; jira_require_env && jira_board get' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira board get"* ]]
}

@test "board issues with no id exits 1 with usage" {
  run bash -c "$S"'; jira_require_env && jira_board issues' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira board issues"* ]]
}
