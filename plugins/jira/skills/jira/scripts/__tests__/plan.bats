#!/usr/bin/env bats
# Tests for lib/plan.sh — the `plan` family dispatcher, its relative sourcing of
# siblings + issue.sh, and the credential exemption for `plan path`. Drives the
# real jira.sh binary so family registration is exercised end to end.

load helpers

setup() {
  setup_sandbox
  export JIRA_PAT=tok
  export JIRA_CLI="$TOOL_DIR/jira.sh"
  REPO="$SANDBOX/repo"; mkdir -p "$REPO"; git -C "$REPO" init -q
  export REPO
}
teardown() { teardown_sandbox; }

@test "dispatch: plan is a registered family and prints its own usage" {
  run bash -c 'cd "$2"; "$1" plan' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"jira plan — decompose ticket work"* ]]
  [[ "$output" != *"unknown family"* ]]
}

@test "dispatch: plan appears in the top-level usage banner" {
  run bash "$TOOL_DIR/jira.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"plan"*"init|run|status|path"* ]]
}

@test "plan path: prints the ledger path with NO credentials configured" {
  unset JIRA_PAT JIRA_BASE_URL
  run bash -c 'cd "$2"; "$1" plan path ABC-123' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -eq 0 ]
  [[ "$output" == */.claude/tmp/plan-ledger/jira-ABC-123.json ]]
}

@test "plan path: still requires a key" {
  run bash -c 'cd "$2"; "$1" plan path' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
}

@test "credential gate still applies to other plan actions" {
  unset JIRA_PAT JIRA_BASE_URL
  run bash -c 'cd "$2"; "$1" plan status ABC-123' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" != *"/.claude/tmp/plan-ledger/"* ]]
}

@test "plan.sh sources issue.sh: jira_issue is callable after sourcing plan.sh alone" {
  # plan init titles the doc from a live `issue get`; jira.sh only sources
  # lib/<family>.sh, so plan.sh must pull issue.sh in itself.
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/adf.sh"; source "$1/lib/paginate.sh"; source "$1/lib/plan.sh"; declare -F jira_issue >/dev/null && echo yes' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [ "$output" = "yes" ]
}

@test "plan.sh sources siblings relatively: works when sourced directly (no \$LIB)" {
  run bash -c 'set -u; source "$1/lib/http.sh"; source "$1/lib/adf.sh"; source "$1/lib/paginate.sh"; source "$1/lib/plan.sh"; declare -F jira_plan_run jira_plan_build jira_plan_parse_steps >/dev/null && echo ok' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [ "$output" = "ok" ]
}

@test "plan run: routes through the family, deriving the key from the doc" {
  cp "$FIXTURES/plan-ok.md" "$REPO/plan.md"
  stub_responses issue.json
  run bash -c 'cd "$2"; "$1" plan run plan.md --step comment-pr-link' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -eq 0 ]
  jq -e '.steps[]|select(.id=="comment-pr-link")|.status=="green"' "$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json" >/dev/null
}

@test "plan run: a doc with no **Issue:** header is rejected" {
  printf '# x\n\n## Steps (machine-readable)\n\n```json\n[{"id":"a","title":"a","check":"true"}]\n```\n' > "$REPO/nokey.md"
  stub_responses issue.json
  run bash -c 'cd "$2"; "$1" plan run nokey.md' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"missing a valid '**Issue:** <KEY>' header"* ]]
}

@test "plan run: needs a doc path" {
  run bash -c 'cd "$2"; "$1" plan run' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"needs a plan doc path"* ]]
}

@test "plan status: reports the summary of a written ledger" {
  cp "$FIXTURES/plan-ok.md" "$REPO/plan.md"
  stub_responses issue.json
  bash -c 'cd "$2"; "$1" plan run plan.md' _ "$TOOL_DIR/jira.sh" "$REPO" || true
  run bash -c 'cd "$2"; "$1" plan status ABC-123' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -eq 0 ]
  [[ "$output" == *"jira-ABC-123"*"1/2 green"* ]]
  [[ "$output" == *"transition-done"* ]]
}

@test "plan status: no ledger yet is an error, not an empty success" {
  run bash -c 'cd "$2"; "$1" plan status ZZZ-9' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"no ledger for ZZZ-9"* ]]
}

@test "plan: an unknown action prints usage" {
  run bash -c 'cd "$2"; "$1" plan bogus' _ "$TOOL_DIR/jira.sh" "$REPO"
  [ "$status" -ne 0 ]
  [[ "$output" == *"jira plan — decompose"* ]]
}
