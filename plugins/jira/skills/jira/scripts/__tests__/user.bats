#!/usr/bin/env bats
# Tests for lib/user.sh — whoami, version-gated user search/lookup query params.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/user.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "user whoami hits /rest/api/3/myself" {
  run bash -c "$S"'; jira_require_env && jira_user whoami' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/myself' "$CURL_LOG"
}

@test "user search -q (v3) hits /user/search?query=bob" {
  run bash -c "$S"'; jira_require_env && jira_user search -q bob' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/user/search?query=bob' "$CURL_LOG"
}

@test "user search -q (v2) hits /user/search?username=bob" {
  export JIRA_API_VERSION=2
  run bash -c "$S"'; jira_require_env && jira_user search -q bob' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/2/user/search?username=bob' "$CURL_LOG"
}

@test "user get (v3) hits /user?accountId=5b10" {
  run bash -c "$S"'; jira_require_env && jira_user get 5b10' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/user?accountId=5b10' "$CURL_LOG"
}

@test "user search URL-encodes a spaced query" {
  run bash -c "$S"'; jira_require_env && jira_user search -q "John Doe"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q 'query=John%20Doe' "$CURL_LOG"
}

@test "user search with no -q exits 1" {
  run bash -c "$S"'; jira_require_env && jira_user search' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira user search"* ]]
}

@test "user unknown action exits 1" {
  run bash -c "$S"'; jira_require_env && jira_user bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira user <whoami|search|get>"* ]]
}

@test "user whoami (live, gated)" {
  [ -n "${JIRA_LIVE:-}" ] || skip "set JIRA_LIVE=1 with real creds to run"
  # Uses real env/curl (no stub PATH) — invoke the real script and assert
  # exit 0 with an accountId in the response.
  run env PATH="$REAL_PATH" bash "$SCRIPTS_DIR/jira.sh" user whoami
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.accountId' >/dev/null
}
