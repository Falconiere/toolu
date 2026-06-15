#!/usr/bin/env bats
# Tests for lib/issue.sh — read, create, comment (ADF-gated), transition
# (name->id), assign (flavor-gated), delete guard. Asserts on the request the
# script builds via the curl stub, plus the lean projection over a real issue.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/adf.sh"; source "$1/lib/issue.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "issue get --lean projects and drops null/empty fields" {
  stub_responses issue.json
  run bash -c "$S"'; jira_require_env && _JIRA_LEAN=1 jira_issue get ABC-123' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.key=="ABC-123" and .status=="In Progress" and .assignee=="Mia Krystof" and (has("resolution")|not) and (has("components")|not)' >/dev/null
}

@test "issue get (non-lean) returns the full body" {
  stub_responses issue.json
  run bash -c "$S"'; jira_require_env && jira_issue get ABC-123' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.fields.summary != null and .fields.resolution == null' >/dev/null
}

@test "issue get with no key exits 1" {
  run bash -c "$S"'; jira_require_env && jira_issue get' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira issue get"* ]]
}

@test "issue create POSTs project/type/summary to the issue endpoint" {
  run bash -c "$S"'; jira_require_env && jira_issue create -p ABC -t Bug -s "boom"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue' "$CURL_LOG"
  grep -q '"project":{"key":"ABC"}' "$CURL_LOG"
  grep -q '"issuetype":{"name":"Bug"}' "$CURL_LOG"
  grep -q '"summary":"boom"' "$CURL_LOG"
}

@test "issue create missing required option exits 1" {
  run bash -c "$S"'; jira_require_env && jira_issue create -p ABC' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira issue create"* ]]
}

@test "issue update PUTs changed fields to the issue endpoint" {
  run bash -c "$S"'; jira_require_env && jira_issue update ABC-1 -s "new title" -f priority=High' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'PUT' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1' "$CURL_LOG"
  grep -q '"summary":"new title"' "$CURL_LOG"
  grep -q '"priority":"High"' "$CURL_LOG"
}

@test "issue update with no key exits 1" {
  run bash -c "$S"'; jira_require_env && jira_issue update' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira issue update"* ]]
}

@test "issue comment wraps text as ADF under v3" {
  run bash -c "$S"'; jira_require_env && jira_issue comment ABC-1 "hi there"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '"type":"doc"' "$CURL_LOG"
  grep -q '"text":"hi there"' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/comment' "$CURL_LOG"
}

@test "issue comment uses a plain body under v2" {
  export JIRA_API_VERSION=2
  run bash -c "$S"'; jira_require_env && jira_issue comment ABC-1 "hi"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '{"body":"hi"}' "$CURL_LOG"
  run grep -q '"type":"doc"' "$CURL_LOG"
  [ "$status" -ne 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/2/issue/ABC-1/comment' "$CURL_LOG"
}

@test "issue transitions lists available transitions via GET" {
  stub_responses transitions.json
  run bash -c "$S"'; jira_require_env && jira_issue transitions ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'GET' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/transitions' "$CURL_LOG"
}

@test "issue transition resolves name to id (case-insensitive) then POSTs it" {
  stub_responses transitions.json transitions.json
  run bash -c "$S"'; jira_require_env && jira_issue transition ABC-1 "in progress"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [ "$(grep -c '/transitions$' "$CURL_LOG")" -ge 2 ]
  grep -q '{"transition":{"id":"21"}}' "$CURL_LOG"
}

@test "issue transition with no match exits 1 listing options" {
  stub_responses transitions.json
  run bash -c "$S"'; jira_require_env && jira_issue transition ABC-1 "Nope"' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"no transition matches"* ]]
  [[ "$output" == *"In Progress"* ]]
}

@test "issue transition with an ambiguous name exits 1 without POSTing" {
  stub_responses transitions-dup.json
  run bash -c "$S"'; jira_require_env && jira_issue transition ABC-1 "Done"' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"ambiguous"* ]]
  run grep -qx 'POST' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "issue assign sends accountId under v3" {
  run bash -c "$S"'; jira_require_env && jira_issue assign ABC-1 5b10acc' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '{"accountId":"5b10acc"}' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/assignee' "$CURL_LOG"
}

@test "issue assign sends name under v2" {
  export JIRA_API_VERSION=2
  run bash -c "$S"'; jira_require_env && jira_issue assign ABC-1 jdoe' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '{"name":"jdoe"}' "$CURL_LOG"
}

@test "issue assign - unassigns with null" {
  run bash -c "$S"'; jira_require_env && jira_issue assign ABC-1 -' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -q '{"accountId":null}' "$CURL_LOG"
}

@test "issue delete DELETEs the issue" {
  run bash -c "$S"'; jira_require_env && jira_issue delete ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'DELETE' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1' "$CURL_LOG"
}

@test "issue delete with no key exits 1" {
  run bash -c "$S"'; jira_require_env && jira_issue delete' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira issue delete"* ]]
}
