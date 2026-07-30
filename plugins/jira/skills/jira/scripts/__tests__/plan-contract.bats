#!/usr/bin/env bats
# Contract test: the ledger this plugin writes must have a stable, well-formed
# shape — schema version, summary/step invariants, and a filename that can
# never collide with toolu's branch-scoped push-gate ledger.
#
# The jira plugin writes schema version 1 itself rather than calling toolu's
# plan-ledger.sh, because pl_ledger_path hardcodes <branch-slug>.json and no env
# var can redirect the filename. That duplication is the cost; this test is the
# alarm.

load helpers

setup() {
  setup_sandbox
  export JIRA_PAT=tok
  export JIRA_CLI="$TOOL_DIR/jira.sh"
  REPO="$SANDBOX/repo"; mkdir -p "$REPO"; git -C "$REPO" init -q
  LEDGER="$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json"
  export REPO LEDGER
  cp "$FIXTURES/plan-ok.md" "$REPO/plan.md"
  stub_responses issue.json
  # One green, one red — exercises both verdict branches in the emitted doc.
  bash -c 'cd "$2"; "$1" plan run plan.md' _ "$TOOL_DIR/jira.sh" "$REPO" || true
}
teardown() { teardown_sandbox; }

@test "contract: base_branch is exactly the empty string" {
  # Load-bearing. state.ts currentDiffSha() opens with `if (!base) return null`,
  # and stale is `sha !== null && ...`. A non-empty base_branch here would make
  # every green Jira card flip amber the moment an unrelated code commit lands.
  jq -e '.base_branch == ""' "$LEDGER" >/dev/null
}

@test "contract: version is 1, matching the push gate's hard schema check" {
  jq -e '.version == 1' "$LEDGER" >/dev/null
}

@test "contract: step status is always one of the four allowed values" {
  jq -e 'all(.steps[]; .status | IN("green","red","pending","running"))' "$LEDGER" >/dev/null
}

@test "contract: summary agrees with steps, and stale is 0" {
  jq -e '
    (.steps|length) as $t
    | .summary.total == $t
    and .summary.green   == (.steps|map(select(.status=="green"))|length)
    and .summary.red     == (.steps|map(select(.status=="red"))|length)
    and .summary.pending == (.steps|map(select(.status=="pending"))|length)
    and .summary.running == (.steps|map(select(.status=="running"))|length)
    and .summary.stale == 0
    and .summary.fresh_green == .summary.green
  ' "$LEDGER" >/dev/null
}

@test "contract: the ledger filename is jira-<KEY>.json, invisible to the push gate" {
  # toolu's push gate stats only "$ledger_dir/<branch-slug>.json". A file named
  # after the issue key can never gate a git push on Jira state.
  [ -f "$REPO/.claude/tmp/plan-ledger/jira-ABC-123.json" ]
  slug=$(git -C "$REPO" rev-parse --abbrev-ref HEAD | tr '/' '-')
  [ ! -f "$REPO/.claude/tmp/plan-ledger/${slug}.json" ]
}

@test "contract: the ledger identifies its project via .branch" {
  jq -e '.branch == "jira-ABC-123"' "$LEDGER" >/dev/null
}
