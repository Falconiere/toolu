#!/usr/bin/env bats
# Tests for hooks/pre-tools/agent-tier.sh — standalone Agent/Task model-tier
# telemetry + advisory hook. Real temp git repos, real ledgers produced by
# actually running plan-ledger.sh, real PreToolUse stdin payloads. No mocks.

bats_require_minimum_version 1.5.0

HOOK="${BATS_TEST_DIRNAME}/../agent-tier.sh"
PLAN_LEDGER="${BATS_TEST_DIRNAME}/../../lib/plan-ledger.sh"

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -b main -q
    git config user.email "t@example.com"
    git config user.name "Tester"
    git commit -q --allow-empty -m init
    git checkout -q -b feat/x
  )
  export PUSH_REVIEW_BASE=main

  # Isolated config dirs: no ambient user/project toolu.config.json leaks in.
  export HOME="$TMP/home"
  unset TOOLU_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME CLAUDE_PROJECT_DIR
  unset TELEMETRY_DIR LEDGER_DIR
  mkdir -p "$HOME/.claude"

  TELEMETRY_FILE="$REPO/.claude/tmp/telemetry/feat_x.jsonl"
  LEDGER_FILE="$REPO/.claude/tmp/plan-ledger/feat_x.json"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Two-step plan; s1's model is haiku, s2's is opus. $1=s1 check $2=s2 check
_write_plan() {
  cat > "$REPO/plan.md" <<EOF
# Fixture Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "First", "check": "$1", "model": "haiku" },
  { "id": "s2", "title": "Second", "check": "$2", "model": "opus" }
]
\`\`\`
EOF
}

@test "tool_name Agent: delegation event appended with model + subagent_type" {
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"sonnet",subagent_type:"implementer"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -f "$TELEMETRY_FILE" ]
  # plan-ledger.sh also writes step_run events (with its own step_id field)
  # into this same per-branch JSONL, so every read below selects the
  # delegation event explicitly rather than assuming it's the only/last line.
  [ "$(jq -r 'select(.event=="delegation") | .event' "$TELEMETRY_FILE" | tail -1)" = "delegation" ]
  [ "$(jq -r 'select(.event=="delegation") | .model' "$TELEMETRY_FILE" | tail -1)" = "sonnet" ]
  [ "$(jq -r 'select(.event=="delegation") | .subagent_type' "$TELEMETRY_FILE" | tail -1)" = "implementer" ]
}

@test "tool_name TaskCreate: no event, exit 0, empty stdout" {
  payload=$(jq -n '{tool_name:"TaskCreate",tool_input:{model:"opus"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "tool_name Task: delegation event appended too (covers both client namings)" {
  payload=$(jq -n '{tool_name:"Task",tool_input:{model:"haiku",subagent_type:"quick-task"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(jq -r 'select(.event=="delegation") | .event' "$TELEMETRY_FILE" | tail -1)" = "delegation" ]
  [ "$(jq -r 'select(.event=="delegation") | .model' "$TELEMETRY_FILE" | tail -1)" = "haiku" ]
}

@test "tool_name spawn_agent: Codex task_name maps to subagent_type telemetry" {
  codex_file="$REPO/.codex/tmp/telemetry/feat_x.jsonl"
  payload=$(jq -n '{tool_name:"spawn_agent",tool_input:{model:"gpt-5.6-terra",task_name:"implementer",reasoning_effort:"medium"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | TOOLU_HOST_OVERRIDE=codex TOOLU_PROJECT_DIR='$REPO' CODEX_HOME='$TMP/codex' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -f "$codex_file" ]
  [ "$(jq -r 'select(.event=="delegation") | .model' "$codex_file" | tail -1)" = "gpt-5.6-terra" ]
  [ "$(jq -r 'select(.event=="delegation") | .subagent_type' "$codex_file" | tail -1)" = "implementer" ]
}

@test "no model param: event has model:null, no additionalContext, exit 0 (inherit is legitimate)" {
  _write_plan "true" "true"
  # --step s1 leaves s2 pending -> plan-ledger.sh correctly exits 1 here; run()
  # so that expected non-zero doesn't abort the test under bats' set -e.
  run bash -c "cd '$REPO' && bash '$PLAN_LEDGER' run plan.md --step s1 >/dev/null"
  # next=s2 (model opus) is now the joined step, but the call carries no model.
  payload=$(jq -n '{tool_name:"Agent",tool_input:{subagent_type:"fork"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  # The --step s1 run above also wrote a step_run event with its own
  # step_id; select the delegation event explicitly rather than assuming
  # it's the only/last line in the JSONL.
  [ "$(jq -r 'select(.event=="delegation") | .model' "$TELEMETRY_FILE" | tail -1)" = "null" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_id' "$TELEMETRY_FILE" | tail -1)" = "s2" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_model' "$TELEMETRY_FILE" | tail -1)" = "opus" ]
}

@test "no ledger at all: step_id/step_model null, no additionalContext" {
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"opus"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_id' "$TELEMETRY_FILE" | tail -1)" = "null" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_model' "$TELEMETRY_FILE" | tail -1)" = "null" ]
}

@test "model mismatch vs running step's model, default advise: additionalContext names both tiers" {
  _write_plan "true" "true"
  run bash -c "cd '$REPO' && bash '$PLAN_LEDGER' run plan.md --step s1 >/dev/null"
  # next=s2 wants opus; call uses sonnet -> mismatch.
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"sonnet"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.hookEventName == "PreToolUse"'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("s2")'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("opus")'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("sonnet")'
  echo "$output" | jq -e '.hookSpecificOutput | has("permissionDecision") | not'
}

@test "model mismatch under agentTier.mode=block: permissionDecision deny" {
  _write_plan "true" "true"
  run bash -c "cd '$REPO' && bash '$PLAN_LEDGER' run plan.md --step s1 >/dev/null"
  echo '{"version":1,"agentTier":{"mode":"block"}}' > "$REPO/.claude/toolu.config.json"
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"sonnet"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("s2")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("opus")'
}

@test "model mismatch under agentTier.mode=off: silent" {
  _write_plan "true" "true"
  run bash -c "cd '$REPO' && bash '$PLAN_LEDGER' run plan.md --step s1 >/dev/null"
  echo '{"version":1,"agentTier":{"mode":"off"}}' > "$REPO/.claude/toolu.config.json"
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"sonnet"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  # Telemetry is still recorded even when the advisory is suppressed. Select
  # the delegation event explicitly — the --step run above also wrote a
  # step_run event with its own step_id into the same JSONL.
  [ "$(jq -r 'select(.event=="delegation") | .step_id' "$TELEMETRY_FILE" | tail -1)" = "s2" ]
}

@test "matching model: no additionalContext, no deny" {
  _write_plan "true" "true"
  run bash -c "cd '$REPO' && bash '$PLAN_LEDGER' run plan.md --step s1 >/dev/null"
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"opus"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "running step (mid --step run) is preferred over next" {
  _write_plan "true" "true"
  ( cd "$REPO" && bash "$PLAN_LEDGER" run plan.md >/dev/null )
  _write_plan "sleep 1; true" "true"
  ( cd "$REPO" && bash "$PLAN_LEDGER" run plan.md --step s1 --activity "checking s1" ) &
  local run_pid=$!

  local saw_running=0 i
  for i in $(seq 1 40); do
    if [ -f "$LEDGER_FILE" ] \
       && [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER_FILE" 2>/dev/null)" = "running" ]; then
      saw_running=1
      payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"sonnet"}}')
      run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
      [ "$status" -eq 0 ]
      # The initial full run above wrote step_run events (s1/s2) into the
      # same JSONL; select the delegation event explicitly.
      [ "$(jq -r 'select(.event=="delegation") | .step_id' "$TELEMETRY_FILE" | tail -1)" = "s1" ]
      [ "$(jq -r 'select(.event=="delegation") | .step_model' "$TELEMETRY_FILE" | tail -1)" = "haiku" ]
      break
    fi
    sleep 0.05
  done
  [ "$saw_running" -eq 1 ]
  wait "$run_pid"
}

@test "no jq: fails open (exit 0, no output, no telemetry)" {
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash git cat mkdir tr date sed wc printf; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c"
  done
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"opus"}}')
  run env -i HOME="$HOME" PATH="$STUB" bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "malformed stdin JSON: fails open (exit 0, no output)" {
  run bash -c "cd '$REPO' && printf 'not-json{' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "unreadable/corrupt ledger: fails open, step_id/step_model null" {
  mkdir -p "$(dirname "$LEDGER_FILE")"
  printf '%s\n' 'this is not json {' > "$LEDGER_FILE"
  payload=$(jq -n '{tool_name:"Agent",tool_input:{model:"opus"}}')
  run bash -c "cd '$REPO' && printf '%s' '$payload' | bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_id' "$TELEMETRY_FILE" | tail -1)" = "null" ]
  [ "$(jq -r 'select(.event=="delegation") | .step_model' "$TELEMETRY_FILE" | tail -1)" = "null" ]
}

@test "hooks.json parses and contains the spawn_agent|Agent|Task entry pointing at agent-tier.sh" {
  HOOKS_JSON="${BATS_TEST_DIRNAME}/../../hooks.json"
  jq -e . "$HOOKS_JSON" >/dev/null
  run jq -e '.hooks.PreToolUse[] | select(.matcher == "spawn_agent|Agent|Task") | .hooks[0].command | test("pre-tools/agent-tier\\.sh$")' "$HOOKS_JSON"
  [ "$status" -eq 0 ]
}
