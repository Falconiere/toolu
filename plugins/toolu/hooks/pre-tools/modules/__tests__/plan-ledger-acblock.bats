#!/usr/bin/env bats
# Tests for the planLedger.blockOnUncoveredAcs promotion in:
#   pre-tools/modules/plan-ledger.sh (spec component 8, AC-8)
#
# Ledgers here are NOT hand-authored — each fixture writes a real plan doc +
# spec doc, then actually runs `bash hooks/lib/plan-ledger.sh run <plan>` so
# the ledger's steps/diff_sha/status come from the real checker, exercising
# the same pl_ac_coverage_lines path pl_cmd_status uses. Real git sandboxes,
# real files, no mocked commands.
#
# Deliberately self-contained (not `load`ing helpers.bash / other __tests__
# bash helper files), mirroring telemetry-sites.bats in this same dir.

# Four levels up from modules/__tests__/ is the dir that CONTAINS hooks/ —
# plugins/toolu (mirrors telemetry-sites.bats).
REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
PLAN_LEDGER_LIB="$REPO_ROOT/hooks/lib/plan-ledger.sh"
PLAN_LEDGER_GATE_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/plan-ledger.sh"

setup() {
  SANDBOX="$(mktemp -d)"
  export SANDBOX
  TELEMETRY_FILE="$SANDBOX/.claude/tmp/telemetry/feat_example.jsonl"

  # Isolated config dirs so the merged config reflects only what each test
  # writes, regardless of the host machine's real ~/.claude config (mirrors
  # telemetry-sites.bats / lib/__tests__/config.bats).
  export HOME="$SANDBOX/home"
  export EMPTY_CFG_DIR="$SANDBOX/agent"
  export TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR"
  export TOOLU_PROJECT_DIR="$SANDBOX"
  mkdir -p "$HOME/.claude" "$EMPTY_CFG_DIR" "$SANDBOX/.claude"

  cd "$SANDBOX" || return 1
  git init -q -b development .
  git config user.email "test@example.com"
  git config user.name "Test"
  echo "base" > base.txt
  git add base.txt
  git commit -q -m "base commit"
  git checkout -q -b feat/example
}

teardown() {
  [ -n "${SANDBOX:-}" ] && rm -rf "$SANDBOX"
}

commit_file() {
  local path="$1" body="${2:-x}"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$body" > "$path"
  git add "$path"
  git commit -q -m "add $path"
}

build_input() {
  jq -n --arg cmd "$1" '{ tool_name: "Bash", tool_input: { command: $cmd } }'
}

# _write_spec SPEC_PATH — a real spec doc declaring AC-1 and AC-2.
_write_spec() {
  cat > "$1" <<'EOF'
# Fixture Spec

## Acceptance criteria

- **AC-1:** first criterion.
- **AC-2:** second criterion.
EOF
}

# _write_plan PLAN_PATH SPEC_PATH_OR_EMPTY STEPS_JSON
# SPEC_PATH_OR_EMPTY empty -> plan carries no **Spec:** field (spec-less).
_write_plan() {
  local plan="$1" spec="$2" steps_json="$3"
  {
    echo "# Fixture Plan"
    echo
    if [ -n "$spec" ]; then
      echo "**Spec:** $spec"
      echo
    fi
    echo "## Steps (machine-readable)"
    echo
    echo '```json'
    printf '%s\n' "$steps_json"
    echo '```'
  } > "$plan"
}

# _write_config BOOL — project-level planLedger.blockOnUncoveredAcs.
_write_config() {
  printf '{"version":1,"planLedger":{"blockOnUncoveredAcs":%s}}' "$1" \
    > "$TOOLU_PROJECT_DIR/.claude/toolu.config.json"
}

# _build_ledger PLAN — actually run the real checker so status/diff_sha come
# from the real mechanism, not a hand-authored ledger.
_build_ledger() {
  local plan="$1"
  PUSH_REVIEW_BASE=development run bash "$PLAN_LEDGER_LIB" run "$plan"
  [ "$status" -eq 0 ]
}

run_plan_ledger_gate() {
  local payload="$1"
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$PLAN_LEDGER_GATE_SCRIPT" <<<"$payload"
}

# _ac_coverage_event -- the last ac_coverage line in TELEMETRY_FILE, or "" if
# none. TELEMETRY_FILE also carries a step_run line from _build_ledger's real
# `plan-ledger.sh run`, so callers must never assume line 1 is theirs.
_ac_coverage_event() {
  [ -f "$TELEMETRY_FILE" ] || return 0
  jq -c 'select(.event == "ac_coverage")' "$TELEMETRY_FILE" | tail -n1
}

# --- (a) blockOnUncoveredAcs=true + an uncovered AC -> deny naming it -------

@test "blockOnUncoveredAcs=true denies push naming the uncovered AC id" {
  commit_file script.sh "echo hi"

  spec="$SANDBOX/spec.md"
  plan="$SANDBOX/plan.md"
  _write_spec "$spec"
  _write_plan "$plan" "$spec" \
    '[ { "id": "s1", "title": "covers AC-1", "check": "true", "ac_refs": ["AC-1"] } ]'
  _build_ledger "$plan"
  _write_config true

  payload=$(build_input "git push")
  run_plan_ledger_gate "$payload"

  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"AC-2"* ]]
  [[ "$reason" != *"AC-1"* ]]

  event=$(_ac_coverage_event)
  [ -n "$event" ]
  [ "$(jq -r '.uncovered' <<<"$event")" = "1" ]
}

# --- (b) default false -> allow; ac_coverage still records uncovered>=1 ----

@test "blockOnUncoveredAcs default false allows push, ac_coverage still records uncovered>=1" {
  commit_file script.sh "echo hi"

  spec="$SANDBOX/spec.md"
  plan="$SANDBOX/plan.md"
  _write_spec "$spec"
  _write_plan "$plan" "$spec" \
    '[ { "id": "s1", "title": "covers AC-1", "check": "true", "ac_refs": ["AC-1"] } ]'
  _build_ledger "$plan"
  # No project config written -> blockOnUncoveredAcs is absent -> default false.

  payload=$(build_input "git push")
  run_plan_ledger_gate "$payload"

  [ "$status" -eq 0 ]
  [ -z "$output" ]

  event=$(_ac_coverage_event)
  [ -n "$event" ]
  uncovered=$(jq -r '.uncovered' <<<"$event")
  [ "$uncovered" -ge 1 ]
}

# --- (c) blockOnUncoveredAcs=true + every AC covered -> allow --------------

@test "blockOnUncoveredAcs=true with every AC covered allows the push" {
  commit_file script.sh "echo hi"

  spec="$SANDBOX/spec.md"
  plan="$SANDBOX/plan.md"
  _write_spec "$spec"
  _write_plan "$plan" "$spec" \
    '[ { "id": "s1", "title": "covers AC-1", "check": "true", "ac_refs": ["AC-1"] },
       { "id": "s2", "title": "covers AC-2", "check": "true", "ac_refs": ["AC-2"] } ]'
  _build_ledger "$plan"
  _write_config true

  payload=$(build_input "git push")
  run_plan_ledger_gate "$payload"

  [ "$status" -eq 0 ]
  [ -z "$output" ]

  event=$(_ac_coverage_event)
  [ -n "$event" ]
  [ "$(jq -r '.covered' <<<"$event")" = "2" ]
  [ "$(jq -r '.uncovered' <<<"$event")" = "0" ]
}

# --- (d) spec-less plan -> never blocks, regardless of config --------------

@test "spec-less plan never blocks even with blockOnUncoveredAcs=true" {
  commit_file script.sh "echo hi"

  plan="$SANDBOX/plan.md"
  _write_plan "$plan" "" '[ { "id": "s1", "title": "no spec", "check": "true" } ]'
  _build_ledger "$plan"
  _write_config true

  payload=$(build_input "git push")
  run_plan_ledger_gate "$payload"

  [ "$status" -eq 0 ]
  [ -z "$output" ]

  # Spec-less: pl_ac_coverage_lines reports nothing, so the gate never even
  # reaches the ac_coverage telemetry call (TELEMETRY_FILE still exists from
  # _build_ledger's step_run event, so assert on event presence, not the file).
  [ -z "$(_ac_coverage_event)" ]
}
