#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/quality-gate.sh

HOOK="${BATS_TEST_DIRNAME}/../quality-gate.sh"

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
  export HOME="$TMP/home"
  export TOOLU_PROJECT_DIR="$TMP"
  export CLAUDE_PROJECT_DIR="$TMP"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE
  mkdir -p "$HOME/.claude" "$TMP/.claude"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  mkdir -p .claude/tmp
  printf '%s\n' '{"status":"failing","reason":"forced","violations":""}' > .claude/tmp/quality-gate-status.json
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "quality-gate: exits 0 with MY_CLAUDE_QUALITY=off" {
  payload='{"tool_input":{"command":"rm -rf /"}}'
  MY_CLAUDE_QUALITY=off tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "quality-gate: exits 0 in a barebones repo with no toolchain (no detected pm + no cargo blocks nothing extra)" {
  # No state file → exit 0 regardless of toolchain.
  rm -f .claude/tmp/quality-gate-status.json
  payload='{"tool_input":{"command":"echo hi"}}'
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# The gate no longer polices arbitrary commands: a failing gate stops shipping,
# not working. `cat tsconfig.json` once had to be denied because the old
# allowlist could be bypassed by substring; there is no allowlist to bypass now.
@test "quality-gate: reading a file is untouched during a failing gate" {
  payload=$(jq -n '{tool_input:{command:"cat tsconfig.json"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "quality-gate: a destructive command is not this gate's business" {
  # Still dangerous, still not what a QUALITY gate is for — the bash denylist
  # and the host permission layer own that decision.
  touch bun.lock
  payload=$(jq -n '{tool_input:{command:"rm -rf node_modules && bun run check"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: `git commit` was in the failing-gate allowlist, defeating the
# gate's stated purpose.
@test "quality-gate: 'git commit -m \"wip\"' is DENIED during failing gate" {
  payload=$(jq -n '{tool_input:{command:"git commit -m \"wip\""}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "quality-gate: 'bun run check' (alone) is ALLOWED during failing gate" {
  touch bun.lock
  payload=$(jq -n '{tool_input:{command:"bun run check"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "quality-gate: 'git status' is ALLOWED during failing gate" {
  payload=$(jq -n '{tool_input:{command:"git status"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: allow_pattern components are joined with `|` at top level.
# Without an outer group, ANCHOR_PREFIX binds only to the FIRST component and
# ANCHOR_SUFFIX only to the LAST — middle components (e.g. the cargo pattern
# in a bun+rust repo) matched anywhere in the string, letting a destructive
# prefix ride along.
@test "quality-gate: a destructive command in a bun+rust repo is also out of scope" {
  command -v bun >/dev/null 2>&1 || skip "bun not installed"
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  touch bun.lock Cargo.toml
  payload=$(jq -n '{tool_input:{command:"rm -rf src && cargo fmt"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "quality-gate: 'cargo fmt' (alone) is ALLOWED in a bun+rust repo" {
  command -v bun >/dev/null 2>&1 || skip "bun not installed"
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  touch bun.lock Cargo.toml
  payload=$(jq -n '{tool_input:{command:"cargo fmt"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Compositional allow case: an allowed first statement + trailing composition.
@test "quality-gate: 'bun run check && bun run build' allowed (first statement matches)" {
  touch bun.lock
  payload=$(jq -n '{tool_input:{command:"bun run check && bun run build"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}


# ── Narrowed scope + modes (AC-9, AC-26) ────────────────────────────────────

gate_config() {
  printf '%s' "$1" > "$TMP/.claude/toolu.config.json"
}

@test "quality-gate: git push is denied during a failing gate" {
  payload=$(jq -n '{tool_input:{command:"git push origin feat/x"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "quality-gate: git -C <path> commit is denied during a failing gate" {
  payload=$(jq -n --arg c "git -C $TMP commit -m \"feat: x\"" '{tool_input:{command:$c}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "quality-gate: a commit reached through && is denied" {
  payload=$(jq -n '{tool_input:{command:"git add -A && git commit -m \"wip\""}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "quality-gate: a commit message that merely mentions pushing is not a push" {
  printf '%s\n' '{"status":"passing"}' > .claude/tmp/quality-gate-status.json
  payload=$(jq -n '{tool_input:{command:"echo ok"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ -z "$output" ]
}

@test "quality-gate: git status is untouched during a failing gate" {
  payload=$(jq -n '{tool_input:{command:"git status"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ -z "$output" ]
}

@test "quality-gate: an Edit tool call is untouched during a failing gate" {
  payload=$(jq -n '{tool_input:{file_path:"/tmp/notes.md"}}')
  tool_name=Edit input="$payload" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "quality-gate: relaxed preset advises instead of denying a push" {
  gate_config '{"version":1,"gates":{"preset":"relaxed"}}'
  payload=$(jq -n '{tool_input:{command:"git push"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  [[ "$ctx" == *"quality gate is failing"* ]]
  # An advisory must not claim it blocked anything.
  [[ "$ctx" != *"BLOCKED"* ]]
}

@test "quality-gate: ask mode phrases the reason as a question" {
  gate_config '{"version":1,"gates":{"qualityGate":{"mode":"ask"}}}'
  payload=$(jq -n '{tool_input:{command:"git push"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"anyway?"* ]]
  [[ "$reason" != *"BLOCKED"* ]]
}

@test "quality-gate: a non-commit non-push command never loads config" {
  # Regression guard for the hot path: this module runs on EVERY Bash call, so
  # a malformed config (which the loader warns about on stderr) must not even
  # be read for a command the gate does not care about.
  printf 'not json' > "$TMP/.claude/toolu.config.json"
  payload=$(jq -n '{tool_input:{command:"ls -la"}}')
  tool_name=Bash input="$payload" run bash "$HOOK" 2>&1
  [ -z "$output" ]
}

@test "quality-gate: per-gate off emits nothing on a push" {
  gate_config '{"version":1,"gates":{"qualityGate":{"mode":"off"}}}'
  payload=$(jq -n '{tool_input:{command:"git push"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  [ -z "$output" ]
}

@test "quality-gate: the failure reason and violations reach the user" {
  printf '%s\n' '{"status":"failing","reason":"oxlint: 2 errors","violations":"src/a.ts:1 no-explicit-any"}' > .claude/tmp/quality-gate-status.json
  payload=$(jq -n '{tool_input:{command:"git push"}}')
  tool_name=Bash input="$payload" run bash "$HOOK"
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"oxlint: 2 errors"* ]]
  [[ "$reason" == *"no-explicit-any"* ]]
}
