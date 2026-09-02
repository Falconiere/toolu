#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/bash-commands.sh

HOOK="${BATS_TEST_DIRNAME}/../bash-commands.sh"

setup() {
  TMP=$(mktemp -d)
  export TOOLU_SETTINGS_DIR="$TMP/settings"
  mkdir -p "$TOOLU_SETTINGS_DIR"

  # Isolated config layers, then pin `block`: these cases are about WHICH
  # commands match a deny rule, not about how the match is delivered. The
  # delivery modes get their own cases at the end of this file.
  export HOME="$TMP/home"
  export TOOLU_PROJECT_DIR="$TMP/project"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE
  mkdir -p "$HOME/.claude" "$TOOLU_PROJECT_DIR/.claude"
  gate_config '{"version":1,"gates":{"bashCommands":{"mode":"block"}}}'
}

gate_config() {
  printf '%s' "$1" > "$TOOLU_PROJECT_DIR/.claude/toolu.config.json"
}

teardown() {
  unset TOOLU_SETTINGS_DIR TOOLU_PROJECT_DIR CLAUDE_PROJECT_DIR
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

write_lists() {
  printf '%s\n' "$1" > "$TOOLU_SETTINGS_DIR/bash-allowlist.txt"
  printf '%s\n' "$2" > "$TOOLU_SETTINGS_DIR/bash-denylist.txt"
}

run_hook() {
  local cmd="$1"
  local payload
  payload=$(jq -n --arg c "$cmd" '{tool_name:"Bash",tool_input:{command:$c}}')
  tool_name=Bash input="$payload" run bash "$HOOK" <<<"$payload"
}

@test "bash-commands: accepts an allowed command (no deny match)" {
  write_lists "" ""
  run_hook "ls -la"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: rejects a denied command (substring match)" {
  write_lists "" "biome"
  run_hook "biome check ."
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: rejects 'node -e' argv-aware even if 'node' alone might be allowed" {
  # node -e is a multi-token argv-aware rule.
  write_lists "" "node -e"
  run_hook 'node -e "console.log(1)"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: rejects 'git push --force origin feat/x'" {
  write_lists "" "git push --force"
  run_hook "git push --force origin feat/x"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: no-op when data files are missing" {
  # Use a settings dir with no lists at all.
  rm -f "$TOOLU_SETTINGS_DIR/bash-allowlist.txt" "$TOOLU_SETTINGS_DIR/bash-denylist.txt"
  run_hook "node -e 'rm -rf /'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: bare 'node script.js' does NOT trip 'node -e' argv rule" {
  write_lists "" "node -e"
  run_hook "node script.js"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Override matrix: deny is evaluated FIRST (so the result carries the deny
# hit), but a matching allowlist entry is an explicit project-specific
# exemption that overrides the deny.
@test "bash-commands: deny-only — denied command with empty allowlist is denied" {
  write_lists "" "biome"
  run_hook "biome check ."
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: deny+allow — allowlist entry overrides the deny (explicit exemption)" {
  write_lists "biome" "biome"
  run_hook "biome check ."
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: deny+allow — argv-aware allow override of 'node -e' deny" {
  write_lists "node -e" "node -e"
  run_hook 'node -e "console.log(1)"'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: deny+allow — allow rule that does not match the command does NOT override" {
  write_lists "biome" "node -e"
  run_hook 'node -e "console.log(1)"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: allow-only — allowlisted command passes" {
  write_lists "ls" ""
  run_hook "ls -la"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: neither list matches — default allow" {
  write_lists "biome" "node -e"
  run_hook "git status"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# `node` on the allowlist is an explicit project exemption — under
# allow-overrides-deny semantics it overrides the `node -e` deny.
@test "bash-commands: deny+allow — broad 'node' allow entry overrides 'node -e' deny" {
  write_lists "node" "node -e"
  run_hook 'node -e "console.log(1)"'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Item 3 regression: empty/whitespace-only command must not crash or
# misbehave when the tokenizer yields no tokens.
@test "bash-commands: empty command with deny rules present is allowed (no crash)" {
  write_lists "" "cargo test"
  run_hook ""
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "bash-commands: whitespace-only command is allowed (empty tokenization falls back safely)" {
  write_lists "" "node -e"
  run_hook "   "
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: `node script.js` IS allowed (no deny match; allow passes through).
@test "bash-commands: 'node script.js' is allowed when node is on allowlist and 'node -e' on denylist" {
  write_lists "node" "node -e"
  run_hook "node script.js"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: heredoc-stripped commit message containing 'cargo test' as
# prose must NOT trip the deny rule. The script's heredoc stripper drops
# the heredoc body before checking.
@test "bash-commands: 'git commit -m \"fix cargo test failure\"' is NOT denied (no argv match)" {
  write_lists "" "cargo test"
  run_hook 'git commit -m "fix cargo test failure"'
  [ "$status" -eq 0 ]
  # `cargo test` is a single-token rule (one word in the rule list after the
  # split), so substring on the command is the fallback. To prevent false
  # positive for single-token rules with internal whitespace, the rule string
  # "cargo test" gets split: first token = "cargo", second = "test", treated
  # as multi-token argv check. tokens[0] = "git", not "cargo" -> no match.
  [ -z "$output" ]
}

# Regression: multi-token deny still fires when argv tokens contain the rule.
@test "bash-commands: 'cargo --verbose test' IS denied via argv rule" {
  write_lists "" "cargo test"
  run_hook "cargo --verbose test"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "bash-commands: 'git push --force origin feat/x' is denied via argv rule" {
  write_lists "" "git push --force"
  run_hook "git push --force origin feat/x"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

# Regression: tokens[0] != "cargo", so `mycargo test` is ALLOWED.
@test "bash-commands: 'mycargo test' is allowed (tokens[0] != cargo)" {
  write_lists "" "cargo test"
  run_hook "mycargo test"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# ── Delivery modes ──────────────────────────────────────────────────────────

@test "bash-commands: the shipped default asks instead of denying or waving through" {
  write_lists "" "node -e"
  gate_config '{"version":1}'
  run_hook 'node -e "console.log(1)"'
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" == *"SECURITY GUARDRAIL"* ]]
}

@test "bash-commands: advise mode still only warns" {
  write_lists "" "node -e"
  gate_config '{"version":1,"gates":{"bashCommands":{"mode":"advise"}}}'
  run_hook 'node -e "console.log(1)"'
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" == *"The command was not stopped"* ]]
}

@test "bash-commands: ask mode prompts instead of denying" {
  write_lists "" "node -e"
  gate_config '{"version":1,"gates":{"bashCommands":{"mode":"ask"}}}'
  run_hook 'node -e "console.log(1)"'
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  local reason
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"SECURITY GUARDRAIL — OVERRIDE REQUESTED"* ]]
  [[ "$reason" == *"node -e"* ]]
  [[ "$reason" == *"full shell privileges"* ]]
}

@test "bash-commands: strict preset restores the hard deny" {
  write_lists "" "node -e"
  gate_config '{"version":1,"gates":{"preset":"strict"}}'
  run_hook 'node -e "console.log(1)"'
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
}

@test "bash-commands: off turns the denylist into a no-op" {
  write_lists "" "node -e"
  gate_config '{"version":1,"gates":{"bashCommands":{"mode":"off"}}}'
  run_hook 'node -e "console.log(1)"'
  [ -z "$output" ]
}

@test "bash-commands: an allowlisted command is silent whatever the mode" {
  write_lists "node -e" "node -e"
  gate_config '{"version":1}'
  run_hook 'node -e "console.log(1)"'
  [ -z "$output" ]
}

@test "bash-commands: ask degrades to BLOCK on codex (guardrail fails closed)" {
  write_lists "" "node -e"
  mkdir -p "$TOOLU_PROJECT_DIR/.codex"
  printf '%s' '{"version":1,"gates":{"bashCommands":{"mode":"ask"}}}' > "$TOOLU_PROJECT_DIR/.codex/toolu.config.json"
  payload=$(jq -n --arg c 'node -e "x"' '{tool_name:"Bash",tool_input:{command:$c}}')
  tool_name=Bash input="$payload" TOOLU_HOST_OVERRIDE=codex run bash "$HOOK" <<<"$payload"
  # bashCommands is a guardrail: with no way to prompt, it denies rather than
  # letting arbitrary code execution through with a note.
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
}
