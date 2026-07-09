#!/usr/bin/env bats
# Tests for lib/plan-parse.sh — steps-block extraction, jq validation, optional
# field backfill, and `**Issue:**` key extraction. Parses real plan docs from
# fixtures/; no network, so no curl stub is needed.

load helpers

S='source "$1/lib/plan-parse.sh"'

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

@test "parse_steps: extracts the fenced block under the exact heading" {
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$FIXTURES/plan-ok.md"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'length==2 and .[0].id=="comment-pr-link" and .[1].id=="transition-done"' >/dev/null
}

@test "parse_steps: prose after the closing fence is not captured" {
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$FIXTURES/plan-ok.md"
  [ "$status" -eq 0 ]
  [[ "$output" != *"must not be captured"* ]]
}

@test "parse_steps: backfills activity to null, preserves an authored one" {
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$FIXTURES/plan-ok.md"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.[0].activity==null and .[1].activity=="transitioning"' >/dev/null
}

@test "parse_steps: a step missing check is rejected" {
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$FIXTURES/plan-bad-steps.md"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a non-empty array of {id,title,check}"* ]]
}

@test "parse_steps: missing doc errors, prints nothing on stdout" {
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$SANDBOX/nope.md"
  [ "$status" -ne 0 ]
  [[ "$output" == *"plan doc not found"* ]]
}

@test "parse_steps: doc without the heading is rejected" {
  printf '# no steps here\n' > "$SANDBOX/bare.md"
  run bash -c "$S"'; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$SANDBOX/bare.md"
  [ "$status" -ne 0 ]
  [[ "$output" == *"no '## Steps (machine-readable)' json block"* ]]
}

@test "issue_key: reads the packed **Issue:** header field" {
  run bash -c "$S"'; jira_plan_issue_key "$2"' _ "$TOOL_DIR" "$FIXTURES/plan-ok.md"
  [ "$status" -eq 0 ]
  [ "$output" = "ABC-123" ]
}

@test "issue_key: rejects a doc with no Issue header" {
  printf '# t\n\n**Date:** 2026-07-09\n' > "$SANDBOX/nokey.md"
  run bash -c "$S"'; jira_plan_issue_key "$2"' _ "$TOOL_DIR" "$SANDBOX/nokey.md"
  [ "$status" -ne 0 ]
  [[ "$output" == *"missing a valid '**Issue:** <KEY>' header"* ]]
}

@test "issue_key: rejects a malformed key (path traversal cannot reach the filename)" {
  printf '# t\n\n**Issue:** ../../etc/passwd   **Topic:** x\n' > "$SANDBOX/evil.md"
  run bash -c "$S"'; jira_plan_issue_key "$2"' _ "$TOOL_DIR" "$SANDBOX/evil.md"
  [ "$status" -ne 0 ]
}

@test "doc_field: a trailing field runs to end of line" {
  run bash -c "$S"'; jira_plan_doc_field "$2" Topic' _ "$TOOL_DIR" "$FIXTURES/plan-ok.md"
  [ "$status" -eq 0 ]
  [ "$output" = "move the ticket to Done and record the PR link" ]
}
