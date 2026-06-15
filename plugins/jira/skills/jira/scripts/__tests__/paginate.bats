#!/usr/bin/env bats
# Tests for lib/paginate.sh — the two pagination models, driven by recorded
# real multi-page search responses under fixtures/.

load helpers

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "paginate: token mode follows nextPageToken until isLast (v3)" {
  stub_responses search-v3-p1.json search-v3-p2.json
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/paginate.sh"; jira_require_env && jira_paginate token POST /rest/api/3/search/jql issues "{\"maxResults\":2}"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==3' >/dev/null
  [ "$(grep -c -- '-X' "$CURL_LOG")" -eq 2 ]
  grep -q 'CAEaBjEwMDAy' "$CURL_LOG"
}

@test "paginate: offset mode follows startAt until total exhausted (v2 GET)" {
  stub_responses search-v2-p1.json search-v2-p2.json
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/paginate.sh"; JIRA_API_VERSION=2 jira_require_env && jira_paginate offset GET /rest/api/2/search issues' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==3' >/dev/null
  [ "$(grep -c 'startAt=' "$CURL_LOG")" -eq 2 ]
  grep -q 'startAt=2' "$CURL_LOG"
}

@test "paginate: single page (isLast on first) makes one request" {
  stub_responses search-v3-p2.json
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/paginate.sh"; jira_require_env && jira_paginate token POST /rest/api/3/search/jql issues "{}"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==1' >/dev/null
  [ "$(grep -c -- '-X' "$CURL_LOG")" -eq 1 ]
}
