#!/usr/bin/env bats
# session-start.sh publishes the jev wrapper at a stable path the agent's Bash
# tool can reach without $CLAUDE_PLUGIN_ROOT, on either host.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  unset TOOLU_HOST_OVERRIDE PLUGIN_ROOT TOOLU_CONFIG_DIR TYPESAFE_API_KEY
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  SRC="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../skills/jev/scripts" && pwd)/jev.sh"
}

teardown() {
  unset TOOLU_HOST_OVERRIDE CODEX_HOME TOOLU_CONFIG_DIR
  rm -rf "$TMP"
}

@test "session-start: publishes wrapper symlink at \$CLAUDE_CONFIG_DIR/jev/jev.sh" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  dst="$CLAUDE_CONFIG_DIR/jev/jev.sh"
  [ -L "$dst" ]
  [ "$(readlink "$dst")" = "$SRC" ]
  [ -x "$dst" ]
}

@test "session-start: publishes under \$CODEX_HOME when the host is codex" {
  export TOOLU_HOST_OVERRIDE=codex
  export CODEX_HOME="$TMP/codex"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$CODEX_HOME/jev/jev.sh")" = "$SRC" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}

@test "session-start: refreshes a stale symlink to the current target" {
  pub_root="$CLAUDE_CONFIG_DIR/jev"
  mkdir -p "$pub_root"
  ln -s /nonexistent/old/jev.sh "$pub_root/jev.sh"
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$pub_root/jev.sh")" = "$SRC" ]
}

@test "session-start: NEVER clobbers a real file at the wrapper path" {
  pub_root="$CLAUDE_CONFIG_DIR/jev"
  mkdir -p "$pub_root"
  printf '#!/usr/bin/env bash\necho user-override\n' > "$pub_root/jev.sh"
  chmod +x "$pub_root/jev.sh"
  before=$(cat "$pub_root/jev.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ ! -L "$pub_root/jev.sh" ]
  [ "$(cat "$pub_root/jev.sh")" = "$before" ]
}

@test "session-start: injects a mandatory workflow and stable path on the first session" {
  export TYPESAFE_API_KEY=local-test-key
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  context=$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")
  [ "$(jq -r '.hookSpecificOutput.hookEventName' <<<"$output")" = SessionStart ]
  [[ "$context" == *"mandatory on every task"* ]]
  [[ "$context" == *"MUST call \"$CLAUDE_CONFIG_DIR/jev/jev.sh\""* ]]
  [[ "$context" == *"Batch independent"* ]]
  # The rule is unconditional per task: the no-decision clause is present and
  # the old "only when it would change" escape hatch is gone.
  [[ "$context" == *"say so in one sentence rather than skipping silently"* ]]
  [[ "$context" != *"only when"* ]]
  [[ "$context" != *local-test-key* ]]
}

@test "session-start: missing key gives actionable fallback without requiring a broken CLI" {
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  context=$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")
  [[ "$context" == *TYPESAFE_API_KEY* ]]
  [[ "$context" == *fallback* ]]
  [[ "$context" == *"mandatory on every task once available"* ]]
  [[ "$context" != *"MUST call"* ]]
}

@test "session-start: native Codex environment injects its path with spaces" {
  export PLUGIN_ROOT="${HOOK%/hooks/session-start.sh}"
  export CODEX_HOME="$TMP/codex profile"
  export TYPESAFE_API_KEY=local-test-key
  run bash "$HOOK" <<<'{"source":"compact"}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$CODEX_HOME/jev/jev.sh")" = "$SRC" ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"$CODEX_HOME/jev/jev.sh"* ]]
}

@test "session-start: paths with quotes and newlines remain JSON string data" {
  installed="$TMP/"$'plugin "cache"\nfolder'
  mkdir -p "$installed"
  cp -R "${HOOK%/hooks/session-start.sh}/." "$installed/"
  export TOOLU_CONFIG_DIR="$TMP/"$'profile "quoted"\nfolder'
  export TYPESAFE_API_KEY=local-test-key
  run bash "$installed/hooks/session-start.sh" <<<'{}'
  [ "$status" -eq 0 ]
  # Context names both the callable wrapper and the SKILL.md syntax reference.
  jq -e --arg wrapper "$TOOLU_CONFIG_DIR/jev/jev.sh" --arg skill_path "$installed/skills/jev/SKILL.md" '
    .hookSpecificOutput | .hookEventName == "SessionStart" and
    (.additionalContext | contains("\"" + $wrapper + "\"") and contains($skill_path + " for syntax"))' <<<"$output"
  # The published wrapper separately links to the installed executable.
  [ "$(readlink "$TOOLU_CONFIG_DIR/jev/jev.sh")" = "$installed/skills/jev/scripts/jev.sh" ]
}

@test "session-start: explicit config directory takes precedence on both hosts" {
  export TOOLU_CONFIG_DIR="$TMP/custom profile"
  export TOOLU_HOST_OVERRIDE=codex
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  [ "$(readlink "$TOOLU_CONFIG_DIR/jev/jev.sh")" = "$SRC" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}

@test "session-start: manifest command runs from an installed plugin path containing spaces" {
  plugin_root="${HOOK%/hooks/session-start.sh}"
  installed="$TMP/plugin cache/jev"
  mkdir -p "$installed"
  cp -R "$plugin_root/." "$installed/"
  export CLAUDE_PLUGIN_ROOT="$installed"
  hook_command=$(jq -r '.hooks.SessionStart[0].hooks[0].command' "$installed/hooks/hooks.json")
  run bash -c "$hook_command" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [ -x "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}

@test "session-start: idempotent (second run leaves the symlink identical)" {
  bash "$HOOK" <<<'{}'
  before=$(readlink "$CLAUDE_CONFIG_DIR/jev/jev.sh")
  run bash "$HOOK" <<<'{}'
  [ "$status" -eq 0 ]
  after=$(readlink "$CLAUDE_CONFIG_DIR/jev/jev.sh")
  [ "$before" = "$after" ]
}

# Fail-soft: a corrupted install where skills/ is missing must NOT break the
# session.
@test "session-start: source wrapper missing -> exits 0, no symlink, silent (fail-soft)" {
  fake="$TMP/fake-plugin/hooks"
  mkdir -p "$fake"
  cp "$HOOK" "$fake/session-start.sh"
  run bash "$fake/session-start.sh" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}

# Fail-soft: skills/ present but hooks/lib/ missing (partial install) must NOT
# break the session either — the guard exits before sourcing.
@test "session-start: shared lib missing -> exits 0, no symlink, silent (fail-soft)" {
  fake="$TMP/fake-plugin"
  mkdir -p "$fake/hooks" "$fake/skills/jev/scripts"
  cp "$HOOK" "$fake/hooks/session-start.sh"
  cp "$SRC" "$fake/skills/jev/scripts/jev.sh"
  export TYPESAFE_API_KEY=local-test-key
  run bash "$fake/hooks/session-start.sh" <<<'{}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -e "$CLAUDE_CONFIG_DIR/jev/jev.sh" ]
}
