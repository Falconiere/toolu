#!/usr/bin/env bats
# Tests for lib/http.sh — env/auth resolution and the curl wrapper.
# The lib is sourced directly; a curl stub on PATH records argv to curl.log.

load helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

@test "http: missing credentials exits 1 naming the vars, no curl" {
  run bash -c 'source "$1/lib/http.sh"; jira_require_env' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"JIRA_PAT"* ]]
  [[ "$output" == *"JIRA_EMAIL"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "http: JIRA_BASE_URL unset exits 1 naming it, no curl" {
  unset JIRA_BASE_URL
  export JIRA_PAT=tok
  run bash -c 'source "$1/lib/http.sh"; jira_require_env' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"JIRA_BASE_URL"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "http: JIRA_PAT yields Bearer auth and no -u" {
  export JIRA_PAT=tok
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/issue/ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'Authorization: Bearer tok' "$CURL_LOG"
  run grep -qx -- '-u' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "http: email+token yields basic -u and no Bearer header" {
  export JIRA_EMAIL='me@x.com' JIRA_API_TOKEN='token'
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/issue/ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx -- '-u' "$CURL_LOG"
  grep -qx 'me@x.com:token' "$CURL_LOG"
  run grep -q 'Authorization: Bearer' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "http: trailing slash on base URL is stripped (no double slash)" {
  export JIRA_PAT=tok JIRA_BASE_URL='https://acme.atlassian.net/'
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/myself' "$CURL_LOG"
  run grep -q '//rest' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "http: default api version is 3" {
  export JIRA_PAT=tok
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && echo "VER=$_JIRA_VER"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"VER=3"* ]]
}

@test "http: JIRA_API_VERSION=2 is honored" {
  export JIRA_PAT=tok JIRA_API_VERSION=2
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && echo "VER=$_JIRA_VER"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"VER=2"* ]]
}

@test "http: invalid api version exits 1" {
  export JIRA_PAT=tok JIRA_API_VERSION=9
  run bash -c 'source "$1/lib/http.sh"; jira_require_env' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"must be 2 or 3"* ]]
}

@test "http: jira_lean pretty-prints by default, projects when _JIRA_LEAN=1" {
  run bash -c 'printf "%s" "{\"a\":1,\"b\":2}" | { source "$1/lib/http.sh"; jira_lean "{a}"; }' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.a==1 and .b==2' >/dev/null
  run bash -c 'printf "%s" "{\"a\":1,\"b\":2}" | { source "$1/lib/http.sh"; _JIRA_LEAN=1 jira_lean "{a}"; }' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.a==1 and (has("b")|not)' >/dev/null
}
