#!/usr/bin/env bats
# Tests for lib/search.sh — flavor-split endpoint + pagination, driven by
# recorded real multi-page search responses.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/paginate.sh"; source "$1/lib/search.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "search v3 POSTs JQL unmodified to /search/jql" {
  stub_responses search-v3-p1.json
  run bash -c "$S"'; jira_require_env && jira_search -q "project=ABC AND status=Open"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/search/jql' "$CURL_LOG"
  grep -q '"jql":"project=ABC AND status=Open"' "$CURL_LOG"
}

@test "search v2 POSTs to /rest/api/2/search" {
  export JIRA_API_VERSION=2
  stub_responses search-v2-p1.json
  run bash -c "$S"'; jira_require_env && jira_search -q "project=ABC"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/2/search' "$CURL_LOG"
  grep -q '"jql":"project=ABC"' "$CURL_LOG"
}

@test "search --all (v3) follows nextPageToken across pages" {
  stub_responses search-v3-p1.json search-v3-p2.json
  run bash -c "$S"'; jira_require_env && jira_search -q "x" --all' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==3' >/dev/null
  [ "$(grep -c -- '-X' "$CURL_LOG")" -eq 2 ]
}

@test "search --all (v2) follows startAt across pages" {
  export JIRA_API_VERSION=2
  stub_responses search-v2-p1.json search-v2-p2.json
  run bash -c "$S"'; jira_require_env && jira_search -q "x" --all' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==3' >/dev/null
  grep -q '"startAt":2' "$CURL_LOG"
}

@test "search --fields includes a fields array in the body" {
  stub_responses search-v3-p1.json
  run bash -c "$S"'; jira_require_env && jira_search -q "x" --fields summary,status' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '"fields":\["summary","status"\]' "$CURL_LOG"
}

@test "search with no JQL exits 1" {
  run bash -c "$S"'; jira_require_env && jira_search' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira search"* ]]
}
