#!/usr/bin/env bats
# Tests for lib/project.sh — list/get/versions/components endpoint shapes,
# version gating, and required-arg/unknown-action guards. Asserts on the exact
# URL the script hands the curl stub.

load helpers

S='source "$1/lib/http.sh"; source "$1/lib/project.sh"'

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "project list hits the v3 project collection" {
  run bash -c "$S"'; jira_require_env && jira_project list' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/project' "$CURL_LOG"
  grep -qx 'GET' "$CURL_LOG"
}

@test "project get <KEY> hits the project detail endpoint" {
  run bash -c "$S"'; jira_require_env && jira_project get ABC' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/project/ABC' "$CURL_LOG"
}

@test "project versions <KEY> hits the versions endpoint" {
  run bash -c "$S"'; jira_require_env && jira_project versions ABC' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/project/ABC/versions' "$CURL_LOG"
}

@test "project components <KEY> hits the components endpoint" {
  run bash -c "$S"'; jira_require_env && jira_project components ABC' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/project/ABC/components' "$CURL_LOG"
}

@test "project list honors JIRA_API_VERSION=2" {
  export JIRA_API_VERSION=2
  run bash -c "$S"'; jira_require_env && jira_project list' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/2/project' "$CURL_LOG"
}

@test "project get with no key exits 1" {
  run bash -c "$S"'; jira_require_env && jira_project get' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira project get"* ]]
}

@test "project unknown action exits 1 with usage" {
  run bash -c "$S"'; jira_require_env && jira_project bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira project <list|get|versions|components>"* ]]
}
