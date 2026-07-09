#!/usr/bin/env bats
# Tests for lib/plan-store.sh — ledger path, document build/merge, summary
# recompute, atomic write. Builds ledgers from the real plan-ok.md fixture.
# base_branch=="" and diff_sha==null are asserted by name: they are what stop
# the dashboard from marking every Jira card stale.

load helpers

S='source "$1/lib/plan-parse.sh"; source "$1/lib/plan-store.sh"'

setup() {
  setup_sandbox
  REPO="$SANDBOX/repo"
  mkdir -p "$REPO"
  git -C "$REPO" init -q
  export REPO
}
teardown() { teardown_sandbox; }

# Build a ledger from the fixture doc inside $REPO and print it.
build() {
  bash -c "$S"'; cd "$3"; steps=$(jira_plan_parse_steps "$2"); jira_plan_build ABC-123 "$2" "$steps" "${4:-null}"' \
    _ "$TOOL_DIR" "$FIXTURES/plan-ok.md" "$REPO" "${1:-null}"
}

@test "ledger_path: jira-<KEY>.json under the repo root, never the branch slug" {
  run bash -c "$S"'; cd "$2"; jira_plan_ledger_path ABC-123' _ "$TOOL_DIR" "$REPO"
  [ "$status" -eq 0 ]
  [ "$output" = "$(cd "$REPO" && pwd -P)/.claude/tmp/plan-ledger/jira-ABC-123.json" ]
}

@test "ledger_path: refuses an empty key" {
  run bash -c "$S"'; jira_plan_ledger_path ""' _ "$TOOL_DIR"
  [ "$status" -ne 0 ]
}

@test "doc_path: <repo>/.claude/tmp/jira/plans/<KEY>.md" {
  run bash -c "$S"'; cd "$2"; jira_plan_doc_path ABC-123' _ "$TOOL_DIR" "$REPO"
  [ "$status" -eq 0 ]
  [[ "$output" == */.claude/tmp/jira/plans/ABC-123.md ]]
}

@test "build: emits schema version 1 with branch jira-<KEY>" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.version==1 and .branch=="jira-ABC-123"' >/dev/null
}

@test "build: base_branch is exactly the empty string (disables dashboard staleness)" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.base_branch == ""' >/dev/null
}

@test "build: every step carries diff_sha null" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'all(.steps[]; .diff_sha == null)' >/dev/null
}

@test "build: fresh steps are pending with the full step field set" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '
    (.steps|length)==2
    and all(.steps[]; .status=="pending")
    and (.steps[0] | has("id") and has("title") and has("check") and has("status")
         and has("started_at") and has("activity") and has("exit_code")
         and has("diff_sha") and has("last_run") and has("evidence_tail"))' >/dev/null
}

@test "build: summary agrees with steps; stale is 0 and fresh_green tracks green" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.summary.total==2 and .summary.pending==2 and .summary.green==0
    and .summary.stale==0 and .summary.fresh_green==.summary.green' >/dev/null
}

@test "build: next is the first non-green step" {
  run build
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.next=="comment-pr-link"' >/dev/null
}

@test "build: carries persisted status forward by step id, authored title wins" {
  prev='{"steps":[{"id":"comment-pr-link","status":"green","exit_code":0,"last_run":"2026-07-09T10:00:00Z","title":"stale title"}]}'
  run build "$prev"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '
    (.steps[]|select(.id=="comment-pr-link")|.status=="green" and .exit_code==0
      and .title=="Comment the PR link on ABC-123")
    and (.steps[]|select(.id=="transition-done")|.status=="pending")' >/dev/null
  echo "$output" | jq -e '.summary.green==1 and .next=="transition-done"' >/dev/null
}

@test "build: all steps green => next is null" {
  prev='{"steps":[{"id":"comment-pr-link","status":"green"},{"id":"transition-done","status":"green"}]}'
  run build "$prev"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.next==null and .summary.green==2 and .summary.fresh_green==2' >/dev/null
}

@test "set_step: merges fields into one step and recomputes the summary" {
  ledger=$(build)
  run bash -c "$S"'; jira_plan_set_step "$2" transition-done "{\"status\":\"red\",\"exit_code\":1}"' _ "$TOOL_DIR" "$ledger"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '(.steps[]|select(.id=="transition-done")|.status=="red" and .exit_code==1)
    and .summary.red==1 and .summary.pending==1' >/dev/null
}

@test "write: is atomic, creates the dir, and leaves no temp file" {
  ledger=$(build)
  target="$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json"
  run bash -c "$S"'; jira_plan_write "$2" "$3"' _ "$TOOL_DIR" "$target" "$ledger"
  [ "$status" -eq 0 ]
  [ -f "$target" ]
  jq -e '.version==1' "$target" >/dev/null
  [ -z "$(find "$(dirname "$target")" -name '*.tmp.*' -print -quit)" ]
}

@test "write: refuses invalid json and writes nothing" {
  target="$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json"
  run bash -c "$S"'; jira_plan_write "$2" "not json"' _ "$TOOL_DIR" "$target"
  [ "$status" -ne 0 ]
  [ ! -f "$target" ]
  [[ "$output" == *"refusing to write invalid ledger json"* ]]
}

@test "read: absent or corrupt ledger returns non-zero with no stdout" {
  run bash -c "$S"'; jira_plan_read "$2/nope.json"' _ "$TOOL_DIR" "$REPO"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
  printf 'not json' > "$REPO/bad.json"
  run bash -c "$S"'; jira_plan_read "$2/bad.json"' _ "$TOOL_DIR" "$REPO"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

@test "read: round-trips what write persisted" {
  ledger=$(build)
  target="$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json"
  bash -c "$S"'; jira_plan_write "$2" "$3"' _ "$TOOL_DIR" "$target" "$ledger"
  run bash -c "$S"'; jira_plan_read "$2"' _ "$TOOL_DIR" "$target"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.branch=="jira-ABC-123" and .base_branch==""' >/dev/null
}

@test "evidence: encodes as a json string, tail-capped" {
  run bash -c "$S"'; jira_plan_evidence "$(printf "l%s\n" 1 2 3 4 5 6 7 8 9 10 11 12)"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e 'type=="string" and (contains("l12")) and (contains("l1\n")|not)' >/dev/null
}
