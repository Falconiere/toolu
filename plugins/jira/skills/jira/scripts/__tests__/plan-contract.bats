#!/usr/bin/env bats
# Contract test: the ledger this plugin writes must satisfy the shape the toolu
# dashboard reads (plugins/toolu/dashboard/state.ts).
#
# The jira plugin writes schema version 1 itself rather than calling toolu's
# plan-ledger.sh, because pl_ledger_path hardcodes <branch-slug>.json and no env
# var can redirect the filename. That duplication is the cost; this test is the
# alarm. If state.ts ever declares a field we do not emit, these tests go red
# instead of the dashboard silently failing to render a card.
#
# state.ts is read via $BATS_TEST_DIRNAME so the test is cwd-independent, and the
# whole file `skip`s when toolu is absent — the jira plugin stays standalone.

load helpers

STATE_TS="$BATS_TEST_DIRNAME/../../../../../toolu/dashboard/state.ts"

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

# Field names declared by a TS interface: match `  name:` / `  name?:` and cut at
# the `?`/`:`. Ignores the `"green" | "red"` union and Record<string, number>,
# which never start a line.
iface_fields() {
  awk -v want="^export interface $1 \\{" '
    $0 ~ want { f = 1; next }
    f && /^\}/ { exit }
    f && match($0, /^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[?]?[[:space:]]*:/) {
      s = $0; sub(/^[[:space:]]*/, "", s); sub(/[?]?[[:space:]]*:.*/, "", s); print s
    }
  ' "$STATE_TS"
}

@test "contract: the emitted ledger has every field state.ts Ledger declares" {
  [ -f "$STATE_TS" ] || skip "toolu dashboard not present"
  [ -f "$LEDGER" ]
  fields=$(iface_fields Ledger)
  [ -n "$fields" ]
  for f in $fields; do
    jq -e --arg f "$f" 'has($f)' "$LEDGER" >/dev/null || { echo "ledger missing field: $f"; return 1; }
  done
}

@test "contract: every emitted step has every field state.ts LedgerStep declares" {
  [ -f "$STATE_TS" ] || skip "toolu dashboard not present"
  fields=$(iface_fields LedgerStep)
  [ -n "$fields" ]
  for f in $fields; do
    jq -e --arg f "$f" 'all(.steps[]; has($f))' "$LEDGER" >/dev/null || { echo "step missing field: $f"; return 1; }
  done
}

@test "contract: base_branch is exactly the empty string" {
  # Load-bearing. state.ts currentDiffSha() opens with `if (!base) return null`,
  # and stale is `sha !== null && ...`. A non-empty base_branch here would make
  # every green Jira card flip amber the moment an unrelated code commit lands.
  jq -e '.base_branch == ""' "$LEDGER" >/dev/null
}

@test "contract: version is 1, matching the push gate's hard schema check" {
  jq -e '.version == 1' "$LEDGER" >/dev/null
}

@test "contract: step status is always one of the four the kanban columnizes" {
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

@test "contract: discovery sees the ledger as its own project via .branch" {
  # discovery.ts makeProject() labels the card from the ledger's .branch field.
  jq -e '.branch == "jira-ABC-123"' "$LEDGER" >/dev/null
}
