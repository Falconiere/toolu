#!/usr/bin/env bats
# Tests for hooks/lib/gate-mode.sh — mode resolution and JSON delivery.

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME
  unset TOOLU_HOST_OVERRIDE PLUGIN_ROOT
  mkdir -p "$HOME/.claude" "$CLAUDE_PROJECT_DIR/.claude"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../gate-mode.sh
  . "$REPO_ROOT/hooks/lib/gate-mode.sh"
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

project_config() {
  printf '%s' "$1" > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  TOOLU_CFG_LOADED=0
}

# ── Defaults (AC-1) ─────────────────────────────────────────────────────────

@test "no config: pushReview is advise" {
  run toolu_gate_mode pushReview
  [ "$status" -eq 0 ]
  [ "$output" = "advise" ]
}

@test "resolving a known gate writes nothing to stderr" {
  # `run` merges stderr into $output, so the assertions above would not notice a
  # stray warning riding along with a correct value. Assert the clean path is
  # actually silent.
  mode=$(toolu_gate_mode pushReview 2>"$TMP/warn")
  [ "$mode" = "advise" ]
  [ ! -s "$TMP/warn" ]
}

@test "no config: qualityGate is block" {
  run toolu_gate_mode qualityGate
  [ "$output" = "block" ]
}

@test "no config: commitGate is advise" {
  run toolu_gate_mode commitGate
  [ "$output" = "advise" ]
}

@test "no config: bashCommands asks (guardrail default)" {
  run toolu_gate_mode bashCommands
  [ "$output" = "ask" ]
}

@test "no config: protectedFiles asks (guardrail default)" {
  run toolu_gate_mode protectedFiles
  [ "$output" = "ask" ]
}

@test "no config: mcpBlocker asks (guardrail default)" {
  run toolu_gate_mode mcpBlocker
  [ "$output" = "ask" ]
}

@test "no config: preset resolves to balanced" {
  run toolu_gate_preset
  [ "$output" = "balanced" ]
}

# ── Presets (AC-2, AC-23) ───────────────────────────────────────────────────

@test "strict preset blocks every gate" {
  project_config '{"version":1,"gates":{"preset":"strict"}}'
  for gate in pushReview qualityGate commitGate bashCommands planLedger docsSync agentTier protectedFiles mcpBlocker; do
    run toolu_gate_mode "$gate"
    [ "$output" = "block" ]
  done
}

@test "relaxed preset advises the judgement gates, turns the soft ones off, and STILL asks on guardrails" {
  project_config '{"version":1,"gates":{"preset":"relaxed"}}'
  run toolu_gate_mode pushReview
  [ "$output" = "advise" ]
  run toolu_gate_mode qualityGate
  [ "$output" = "advise" ]
  run toolu_gate_mode commitGate
  [ "$output" = "off" ]
  run toolu_gate_mode planLedger
  [ "$output" = "off" ]
  run toolu_gate_mode docsSync
  [ "$output" = "off" ]
  # relaxed means "stop lecturing me", not "write my .env without telling me".
  run toolu_gate_mode bashCommands
  [ "$output" = "ask" ]
  run toolu_gate_mode protectedFiles
  [ "$output" = "ask" ]
  run toolu_gate_mode mcpBlocker
  [ "$output" = "ask" ]
}

@test "a guardrail can still be turned off explicitly, per gate" {
  project_config '{"version":1,"gates":{"protectedFiles":{"mode":"off"}}}'
  run toolu_gate_mode protectedFiles
  [ "$output" = "off" ]
}

@test "a guardrail can be put back to a hard block explicitly" {
  project_config '{"version":1,"gates":{"protectedFiles":{"mode":"block"}}}'
  run toolu_gate_mode protectedFiles
  [ "$output" = "block" ]
}

# ── Per-gate override beats preset (AC-3) ───────────────────────────────────

@test "per-gate mode overrides the preset" {
  project_config '{"version":1,"gates":{"preset":"strict","pushReview":{"mode":"off"}}}'
  run toolu_gate_mode pushReview
  [ "$output" = "off" ]
  run toolu_gate_mode qualityGate
  [ "$output" = "block" ]
}

@test "user config is overridden by project config" {
  printf '%s' '{"version":1,"gates":{"preset":"strict"}}' > "$HOME/.claude/toolu.config.json"
  project_config '{"version":1,"gates":{"preset":"relaxed"}}'
  run toolu_gate_mode pushReview
  [ "$output" = "advise" ]
}

# ── Legacy keys (AC-24) ─────────────────────────────────────────────────────

@test "gates.docsSync.mode wins over the legacy docsSync.mode" {
  project_config '{"version":1,"docsSync":{"mode":"off"},"gates":{"docsSync":{"mode":"block"}}}'
  run toolu_gate_mode docsSync
  [ "$output" = "block" ]
}

@test "legacy docsSync.mode is honored when gates.docsSync is absent" {
  project_config '{"version":1,"docsSync":{"mode":"off"}}'
  run toolu_gate_mode docsSync
  [ "$output" = "off" ]
}

@test "legacy agentTier.mode is honored" {
  project_config '{"version":1,"agentTier":{"mode":"block"}}'
  run toolu_gate_mode agentTier
  [ "$output" = "block" ]
}

@test "a legacy key on a gate that never had one is ignored" {
  project_config '{"version":1,"pushReview":{"mode":"off"}}'
  run toolu_gate_mode pushReview
  [ "$output" = "advise" ]
}

# ── Bad values (AC-4) ───────────────────────────────────────────────────────

@test "unknown mode warns and falls back to the preset value" {
  project_config '{"version":1,"gates":{"preset":"strict","pushReview":{"mode":"loud"}}}'
  # stdout and stderr are asserted separately: the fallback value is stdout,
  # the typo warning is stderr, and bats `run` would merge the two.
  mode=$(toolu_gate_mode pushReview 2>"$TMP/warn")
  [ "$mode" = "block" ]
  grep -q "not an allowed value" "$TMP/warn"
}

@test "unknown preset warns and falls back to balanced" {
  project_config '{"version":1,"gates":{"preset":"yolo"}}'
  mode=$(toolu_gate_mode pushReview 2>"$TMP/warn")
  [ "$mode" = "advise" ]
  grep -q "not an allowed value" "$TMP/warn"
}

@test "unknown gate name enforces block and returns non-zero" {
  set +e
  mode=$(toolu_gate_mode notAGate 2>"$TMP/warn")
  rc=$?
  set -e
  [ "$rc" -eq 1 ]
  [ "$mode" = "block" ]
  grep -q "unknown gate" "$TMP/warn"
}

# ── Host degrade (AC-8) ─────────────────────────────────────────────────────

@test "a GUARDRAIL's ask degrades to block on codex, never to advise" {
  # The security-critical half of the degrade rule: where no human can be
  # prompted, a guardrail must fail closed. Degrading to advise would hand an
  # agent silent .env access on exactly the hosts nobody is watching.
  export TOOLU_HOST_OVERRIDE=codex
  export TOOLU_PROJECT_DIR="$CLAUDE_PROJECT_DIR"
  mkdir -p "$TOOLU_PROJECT_DIR/.codex"
  printf '%s' '{"version":1}' > "$TOOLU_PROJECT_DIR/.codex/toolu.config.json"
  TOOLU_CFG_LOADED=0
  for gate in protectedFiles mcpBlocker bashCommands; do
    run toolu_gate_mode "$gate"
    [ "$output" = "block" ]
  done
}

@test "ask degrades to advise on codex" {
  # qualityGate defaults to block, so a missed .codex load cannot pass as advise.
  export TOOLU_HOST_OVERRIDE=codex
  export TOOLU_PROJECT_DIR="$CLAUDE_PROJECT_DIR"
  mkdir -p "$TOOLU_PROJECT_DIR/.codex"
  printf '%s' '{"version":1,"gates":{"qualityGate":{"mode":"ask"}}}' > "$TOOLU_PROJECT_DIR/.codex/toolu.config.json"
  TOOLU_CFG_LOADED=0
  run toolu_gate_mode qualityGate
  [ "$output" = "advise" ]
}

@test "block does not degrade on codex" {
  # Codex reads .codex/toolu.config.json under TOOLU_PROJECT_DIR and never
  # consults CLAUDE_PROJECT_DIR, so the fixture uses the real Codex layout.
  export TOOLU_HOST_OVERRIDE=codex
  export TOOLU_PROJECT_DIR="$CLAUDE_PROJECT_DIR"
  mkdir -p "$TOOLU_PROJECT_DIR/.codex"
  printf '%s' '{"version":1,"gates":{"preset":"strict"}}' > "$TOOLU_PROJECT_DIR/.codex/toolu.config.json"
  TOOLU_CFG_LOADED=0
  mode=$(toolu_gate_mode pushReview 2>/dev/null)
  [ "$mode" = "block" ]
}

@test "codex reads its own project config for a per-gate override" {
  export TOOLU_HOST_OVERRIDE=codex
  export TOOLU_PROJECT_DIR="$CLAUDE_PROJECT_DIR"
  mkdir -p "$TOOLU_PROJECT_DIR/.codex"
  printf '%s' '{"version":1,"gates":{"qualityGate":{"mode":"off"}}}' > "$TOOLU_PROJECT_DIR/.codex/toolu.config.json"
  TOOLU_CFG_LOADED=0
  mode=$(toolu_gate_mode qualityGate 2>/dev/null)
  [ "$mode" = "off" ]
}

# ── Emission ────────────────────────────────────────────────────────────────

@test "emit block produces a deny decision carrying the reason" {
  run toolu_gate_emit block "no review recorded"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "no review recorded" ]
}

@test "emit ask produces an ask decision carrying the reason" {
  run toolu_gate_emit ask "push without a review?"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "push without a review?" ]
}

@test "emit advise produces additionalContext and no decision" {
  run toolu_gate_emit advise "docs look stale"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" = "docs look stale" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
}

@test "emit off produces nothing" {
  run toolu_gate_emit off "quiet"
  [ -z "$output" ]
}

@test "emit with an unknown mode falls back to deny" {
  json=$(toolu_gate_emit sideways "unclear" 2>"$TMP/warn")
  [ "$(echo "$json" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
  grep -q "unknown mode" "$TMP/warn"
}

@test "emit preserves multi-line reasons verbatim" {
  run toolu_gate_emit ask "line one
line two"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason' | wc -l | tr -d ' ')" = "2" ]
}
