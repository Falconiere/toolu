#!/usr/bin/env bats
# Tests for lib/plan-run.sh — the whoami probe, the two-write running->verdict
# sequence, evidence capture, and --step/--activity. Checks run against the real
# jira.sh in the sandbox (JIRA_CLI) and hit the curl stub replaying the recorded
# fixtures/issue.json, so `green` really means "the REST assertion held".

load helpers

S='source "$1/lib/plan-parse.sh"; source "$1/lib/plan-store.sh"; source "$1/lib/plan-run.sh"'

setup() {
  setup_sandbox
  export JIRA_PAT=tok
  export JIRA_CLI="$TOOL_DIR/jira.sh"
  REPO="$SANDBOX/repo"; mkdir -p "$REPO"; git -C "$REPO" init -q
  LEDGER="$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json"
  export REPO LEDGER
  # fixtures/issue.json has key ABC-123 and status "In Progress": the doc's first
  # step asserts the key (green), the second asserts status Done (red).
  cp "$FIXTURES/plan-ok.md" "$SANDBOX/plan.md"
  DOC="$SANDBOX/plan.md"; export DOC
}
teardown() { teardown_sandbox; }

runplan() { bash -c "$S"'; cd "$2"; shift 2; jira_plan_run ABC-123 "$@"' _ "$TOOL_DIR" "$REPO" "$@"; }

@test "run: green when Jira agrees, red when it does not; exits non-zero" {
  stub_responses issue.json
  run runplan "$DOC"
  [ "$status" -ne 0 ]
  jq -e '.steps[]|select(.id=="comment-pr-link")|.status=="green" and .exit_code==0' "$LEDGER" >/dev/null
  jq -e '.steps[]|select(.id=="transition-done")|.status=="red" and .exit_code!=0' "$LEDGER" >/dev/null
}

@test "run: writes a v1 ledger at jira-<KEY>.json with base_branch empty" {
  stub_responses issue.json
  run runplan "$DOC"
  [ -f "$LEDGER" ]
  jq -e '.version==1 and .branch=="jira-ABC-123" and .base_branch==""' "$LEDGER" >/dev/null
}

@test "run: summary and next reflect the verdicts" {
  stub_responses issue.json
  run runplan "$DOC"
  jq -e '.summary.green==1 and .summary.red==1 and .summary.stale==0 and .next=="transition-done"' "$LEDGER" >/dev/null
}

@test "run: the step is status=running while its own check executes" {
  # The check reads the ledger the runner just wrote — proving the first of the
  # two writes lands before the check runs (this is what the dashboard renders).
  cat > "$SANDBOX/obs.md" <<'DOC'
# obs

**Issue:** ABC-123   **Topic:** observe our own running state

## Steps (machine-readable)

```json
[{ "id": "obs", "title": "observe running", "check": "jq -e '.steps[]|select(.id==\"obs\")|.status==\"running\" and .started_at!=null' .claude/tmp/plan-ledger/jira-ABC-123.json >/dev/null" }]
```
DOC
  stub_responses issue.json
  run runplan "$SANDBOX/obs.md"
  [ "$status" -eq 0 ]
  jq -e '.steps[0].status=="green" and .steps[0].started_at==null' "$LEDGER" >/dev/null
}

@test "run: probe failure writes NO statuses and leaves an existing ledger untouched" {
  stub_responses issue.json
  runplan "$DOC" || true                       # seed a real ledger
  before=$(cat "$LEDGER")
  export JIRA_STUB_FAIL=22                     # every curl now fails => whoami fails
  run runplan "$DOC"
  [ "$status" -ne 0 ]
  [[ "$output" == *"cannot reach Jira"* ]]
  [[ "$output" == *"no step statuses were written"* ]]
  [ "$(cat "$LEDGER")" = "$before" ]
}

@test "run: probe failure on a fresh repo creates no ledger at all" {
  export JIRA_STUB_FAIL=22
  run runplan "$DOC"
  [ "$status" -ne 0 ]
  [ ! -f "$LEDGER" ]
}

@test "run: --step executes only that step, leaving the other pending" {
  stub_responses issue.json
  run runplan "$DOC" --step comment-pr-link
  [ "$status" -eq 0 ]
  jq -e '.steps[]|select(.id=="comment-pr-link")|.status=="green"' "$LEDGER" >/dev/null
  jq -e '.steps[]|select(.id=="transition-done")|.status=="pending" and .exit_code==null' "$LEDGER" >/dev/null
}

@test "run: --step with an unknown id errors and writes nothing" {
  stub_responses issue.json
  run runplan "$DOC" --step nope
  [ "$status" -ne 0 ]
  [[ "$output" == *"no step 'nope'"* ]]
  [ ! -f "$LEDGER" ]
}

@test "run: --activity labels the executed step" {
  stub_responses issue.json
  run runplan "$DOC" --step comment-pr-link --activity "checking the link"
  jq -e '.steps[]|select(.id=="comment-pr-link")|.activity=="checking the link"' "$LEDGER" >/dev/null
}

@test "run: a red step captures the check's output as evidence_tail" {
  stub_responses issue.json
  run runplan "$DOC" --step transition-done
  [ "$status" -ne 0 ]
  jq -e '.steps[]|select(.id=="transition-done")|.evidence_tail|type=="string"' "$LEDGER" >/dev/null
}

@test "run: an unknown option is rejected" {
  run runplan "$DOC" --bogus x
  [ "$status" -ne 0 ]
  [[ "$output" == *"unknown option '--bogus'"* ]]
}

@test "run: a missing jira CLI is reported, not silently treated as red" {
  export JIRA_CLI="$SANDBOX/absent.sh"
  run runplan "$DOC"
  [ "$status" -ne 0 ]
  [[ "$output" == *"jira CLI not found"* ]]
  [ ! -f "$LEDGER" ]
}

@test "run: re-running a red step flips it green once Jira agrees" {
  stub_responses issue.json
  run runplan "$DOC" --step transition-done
  jq -e '.steps[]|select(.id=="transition-done")|.status=="red"' "$LEDGER" >/dev/null
  # Now Jira reports Done: the same unchanged check must go green on re-run.
  jq '.fields.status.name = "Done"' "$FIXTURES/issue.json" > "$SANDBOX/done.json"
  stub_responses "$SANDBOX/done.json"
  run runplan "$DOC" --step transition-done
  [ "$status" -eq 0 ]
  jq -e '.steps[]|select(.id=="transition-done")|.status=="green" and .exit_code==0' "$LEDGER" >/dev/null
  jq -e '.summary.green==1 and .next=="comment-pr-link"' "$LEDGER" >/dev/null
}
