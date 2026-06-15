#!/usr/bin/env bats
# Tests for lib/raw.sh — the generic escape-hatch request.

load helpers

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "raw: GET hits the exact path with resolved auth and no body" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/raw.sh"; jira_require_env && jira_raw GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/myself' "$CURL_LOG"
  grep -qx 'GET' "$CURL_LOG"
  grep -qx 'Authorization: Bearer tok' "$CURL_LOG"
  run grep -q -- '--data' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "raw: POST sends the body verbatim" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/raw.sh"; jira_require_env && jira_raw POST /x "{\"a\":1}"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/x' "$CURL_LOG"
  grep -q -- '--data' "$CURL_LOG"
  grep -qx '{"a":1}' "$CURL_LOG"
}

@test "raw: a non-zero HTTP failure propagates under pipefail (as the dispatcher runs it)" {
  export JIRA_STUB_FAIL=22
  run bash -c 'set -o pipefail; source "$1/lib/http.sh"; source "$1/lib/raw.sh"; jira_require_env && jira_raw GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -ne 0 ]
}

@test "raw: too few args exits 1 with usage" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/raw.sh"; jira_require_env && jira_raw GET' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira raw"* ]]
}
