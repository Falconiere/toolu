#!/usr/bin/env bats
# Tests for `jira.sh plan init <KEY>` — scaffolds the plan doc, titled from a
# live `issue get` served by the curl stub from the recorded fixtures/issue.json
# (summary: "Login page throws 500 on empty password").

load helpers

setup() {
  setup_sandbox
  export JIRA_PAT=tok
  REPO="$SANDBOX/repo"; mkdir -p "$REPO"; git -C "$REPO" init -q
  DOC="$REPO/.claude/tmp/jira/plans/ABC-123.md"
  export REPO DOC
}
teardown() { teardown_sandbox; }

init() { bash -c 'cd "$2"; "$1" plan init "${3:-ABC-123}"' _ "$TOOL_DIR/jira.sh" "$REPO" "$@"; }

@test "init: writes the doc under .claude/tmp/jira/plans and prints its path" {
  stub_responses issue.json
  run init
  [ "$status" -eq 0 ]
  [ "$output" = "$(cd "$REPO" && pwd -P)/.claude/tmp/jira/plans/ABC-123.md" ]
  [ -f "$DOC" ]
}

@test "init: titles the doc from the live issue summary" {
  stub_responses issue.json
  run init
  [ "$status" -eq 0 ]
  grep -q '^# ABC-123 — Login page throws 500 on empty password$' "$DOC"
}

@test "init: emits the **Issue:** header the runner parses back" {
  stub_responses issue.json
  init
  run bash -c 'source "$1/lib/plan-parse.sh"; jira_plan_issue_key "$2"' _ "$TOOL_DIR" "$DOC"
  [ "$status" -eq 0 ]
  [ "$output" = "ABC-123" ]
}

@test "init: scaffolds an empty machine-readable steps block" {
  stub_responses issue.json
  init
  grep -q '^## Steps (machine-readable)$' "$DOC"
  grep -q '^\[\]$' "$DOC"
}

@test "init: the scaffolded empty block is rejected by the parser until filled" {
  stub_responses issue.json
  init
  run bash -c 'source "$1/lib/plan-parse.sh"; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$DOC"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a non-empty array"* ]]
}

@test "init: guidance shows a REST-assertion check bound to \$JIRA" {
  stub_responses issue.json
  init
  grep -q 'issue get ABC-123 --lean' "$DOC"
  grep -q 'exits 0 \*\*only when Jira itself reflects the change\*\*' "$DOC"
}

@test "init: refuses to clobber an existing doc" {
  stub_responses issue.json issue.json
  init
  printf 'hand-edited\n' > "$DOC"
  run init
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
  [ "$(cat "$DOC")" = "hand-edited" ]
}

@test "init: needs an issue key" {
  stub_responses issue.json
  run bash -c 'cd "$2"; "$1" plan init' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"needs an issue key"* ]]
}

@test "init: end-to-end — scaffold, author a step, run it green" {
  stub_responses issue.json
  init
  # Replace the empty array with a real REST assertion against the same fixture.
  perl -0pi -e 's/\[\]/[{"id":"key-is-right","title":"issue exists","check":"\\"\$JIRA\\" issue get ABC-123 --lean | jq -e '"'"'.key==\\"ABC-123\\"'"'"' >\/dev\/null"}]/' "$DOC"
  run bash -c 'source "$1/lib/plan-parse.sh"; jira_plan_parse_steps "$2"' _ "$TOOL_DIR" "$DOC"
  [ "$status" -eq 0 ]
  export JIRA_CLI="$TOOL_DIR/jira.sh"
  stub_responses issue.json
  run bash -c 'cd "$2"; "$1" plan run .claude/tmp/jira/plans/ABC-123.md' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -eq 0 ]
  jq -e '.steps[0].status=="green" and .base_branch==""' "$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json" >/dev/null
}
