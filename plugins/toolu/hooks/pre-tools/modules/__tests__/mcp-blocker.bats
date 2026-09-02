#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/mcp-blocker.sh

HOOK="${BATS_TEST_DIRNAME}/../mcp-blocker.sh"

setup() {
  TMP=$(mktemp -d)
  export TOOLU_SETTINGS_DIR="$TMP/settings"
  mkdir -p "$TOOLU_SETTINGS_DIR"
  printf '%s\n' "exampleblocked" > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  # TMPHOME is created lazily by tests that need a per-test $HOME sandbox;
  # registering it here lets teardown clean it up even if the test aborts
  # mid-assertion.
  TMPHOME=""
}

teardown() {
  unset TOOLU_SETTINGS_DIR
  if [ -n "${TMP:-}" ] && [ -d "$TMP" ]; then
    rm -rf "$TMP"
  fi
  if [ -n "${TMPHOME:-}" ] && [ -d "$TMPHOME" ]; then
    rm -rf "$TMPHOME"
  fi
}

@test "mcp-blocker: asks on a listed server" {
  tool_name=mcp__exampleblocked__search run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "mcp-blocker: allows an unlisted server" {
  tool_name=mcp__other__do_thing run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: doc comment + settings file both say "PREFIXES", but the
# original code did exact equality. Prefix entries like `claude_ai_` silently
# never matched.
@test "mcp-blocker: prefix entry 'claude_ai_' blocks 'claude_ai_Canva'" {
  printf '%s\n' "claude_ai_" > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  tool_name=mcp__claude_ai_Canva__search run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "mcp-blocker: prefix entry 'claude_ai_' does NOT block 'friend_ai_Canva'" {
  printf '%s\n' "claude_ai_" > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  tool_name=mcp__friend_ai_Canva__search run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "mcp-blocker: exact server name still blocks itself" {
  printf '%s\n' "exampleblocked" > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  tool_name=mcp__exampleblocked__save run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

# Regression: this module is wired as a STANDALONE PreToolUse hook (matcher
# mcp__), so the parent never `export`s tool_name. Claude Code delivers it as
# .tool_name in the JSON on STDIN. With tool_name unset in the env, the gate
# must still read it from stdin and block a listed server.
@test "mcp-blocker: reads tool_name from stdin when env is unset (standalone path)" {
  payload=$(jq -n '{tool_name:"mcp__exampleblocked__search",tool_input:{}}')
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "mcp-blocker: BLOCKS a listed server on Codex, where no prompt is possible" {
  # Same payload a Codex host really sends. Codex cannot deliver an `ask`, and
  # a guardrail fails closed rather than degrading to a note nobody enforces.
  payload=$(jq -n --arg cwd "$TMP" \
    '{session_id:"session-1",turn_id:"turn-1",cwd:$cwd,
      hook_event_name:"PreToolUse",permission_mode:"default",
      tool_name:"mcp__exampleblocked__search",tool_input:{query:"needle"}}')

  TOOLU_HOST_OVERRIDE=codex run bash "$HOOK" <<<"$payload"

  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("exampleblocked")'
}

@test "config.mcp disables a server without touching the blocklist file" {
  TMPHOME=$(mktemp -d)
  mkdir -p "$TMPHOME/.claude"
  cat > "$TMPHOME/.claude/toolu.config.json" <<JSON
{"version":1,"mcp":{"someserver":false}}
JSON

  payload=$(jq -n '{tool_name:"mcp__someserver__do",tool_input:{}}')
  # Four levels up = the dir containing hooks/ — plugins/toolu since the
  # Plan 2 reorg (was the repo root).
  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
  # Pin the agent config dir to the test sandbox via an EXPORTED TOOLU_CONFIG_DIR
  # (highest precedence in _toolu_agent_dir). Two reasons a `run` env-prefix
  # won't do: (1) bash's temporary assignment on the `run` function does not
  # export into the spawned subshell; (2) the ambient CLAUDE_CONFIG_DIR set by a
  # real Claude Code session would otherwise win and point _toolu_user_cfg at
  # the user's real config, not the sandbox. (Each @test is its own subshell, so
  # this export does not leak to sibling tests.)
  export TOOLU_CONFIG_DIR="$TMPHOME/.claude"
  tool_name=mcp__someserver__do \
    run bash "$REPO_ROOT/hooks/pre-tools/modules/mcp-blocker.sh" <<<"$payload"

  [ "$status" -eq 0 ]
  [[ "$output" == *"someserver"* ]]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("toolu config")'
  # TMPHOME cleanup happens in teardown so an assertion failure above does
  # not leak the temp dir.
}

# Regression: a non-object `.mcp` (string/array) made `to_entries` error.
# The `objects` guard must turn it into a no-op instead.
@test "config.mcp as a non-object does not block or crash" {
  TMPHOME=$(mktemp -d)
  mkdir -p "$TMPHOME/.claude"
  cat > "$TMPHOME/.claude/toolu.config.json" <<JSON
{"version":1,"mcp":"broken-not-an-object"}
JSON

  payload=$(jq -n '{tool_name:"mcp__someserver__do",tool_input:{}}')
  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
  tool_name=mcp__someserver__do input="$payload" HOME="$TMPHOME" \
    run bash "$REPO_ROOT/hooks/pre-tools/modules/mcp-blocker.sh" <<<"$payload"

  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "block-from-file deny reason directs user to mcp-blocklist.txt, not config" {
  # File-source block must NOT mention toolu.config.json
  tool_name=mcp__exampleblocked__search run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("mcp-blocklist.txt")'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("toolu config") | not'
}

@test "mcp-blocker: an entry's '-> redirect' hint is appended to the deny reason" {
  printf '%s\n' "someserver -> use the jira skill instead" \
    > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  tool_name=mcp__someserver__do run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains("use the jira skill instead")'
}

@test "mcp-blocker: a '#'-commented entry does not block (nudge-only safety)" {
  # read_list strips comment lines, so a documented-but-commented sample like
  # the Atlassian example must NOT deny — Atlassian stays callable (nudge-only).
  printf '%s\n' "# claude_ai_Atlassian -> use the jira skill instead" \
    > "$TOOLU_SETTINGS_DIR/mcp-blocklist.txt"
  tool_name=mcp__claude_ai_Atlassian__createIssue run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
