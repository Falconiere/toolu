#!/usr/bin/env bats
# Tests for lib/worklog.sh — add (with optional ADF-gated comment), list (lean
# projection), delete, plus arg/usage guards. Asserts on the request the script
# builds via the curl stub.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/adf.sh"; source "$1/lib/worklog.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "worklog add POSTs timeSpent to the worklog endpoint" {
  run bash -c "$S"'; jira_require_env && jira_worklog add ABC-1 -t 1h' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/worklog' "$CURL_LOG"
  grep -q '"timeSpent":"1h"' "$CURL_LOG"
}

@test "worklog add wraps comment as ADF under v3" {
  run bash -c "$S"'; jira_require_env && jira_worklog add ABC-1 -t 1h -c "did work"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '"timeSpent":"1h"' "$CURL_LOG"
  grep -q '"type":"doc"' "$CURL_LOG"
  grep -q '"text":"did work"' "$CURL_LOG"
}

@test "worklog add uses a plain comment string under v2" {
  export JIRA_API_VERSION=2
  run bash -c "$S"'; jira_require_env && jira_worklog add ABC-1 -t 1h -c "did work"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '"comment":"did work"' "$CURL_LOG"
  run grep -q '"type":"doc"' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "worklog add with no -t exits 1" {
  run bash -c "$S"'; jira_require_env && jira_worklog add ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira worklog add"* ]]
}

@test "worklog list GETs the worklog endpoint" {
  run bash -c "$S"'; jira_require_env && jira_worklog list ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/worklog' "$CURL_LOG"
  grep -qx 'GET' "$CURL_LOG"
}

@test "worklog delete DELETEs the worklog entry" {
  run bash -c "$S"'; jira_require_env && jira_worklog delete ABC-1 99' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/worklog/99' "$CURL_LOG"
  grep -qx 'DELETE' "$CURL_LOG"
  [[ "$output" == *"deleted worklog 99"* ]]
}

@test "worklog with unknown action exits 1" {
  run bash -c "$S"'; jira_require_env && jira_worklog bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira worklog <add|list|delete>"* ]]
}
