#!/usr/bin/env bats
# user-prompt-submit.sh restates the Jev mandate on every task, naming the
# published wrapper for the active host, and stays silent where a reminder
# could not be acted on.

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  unset TOOLU_HOST_OVERRIDE PLUGIN_ROOT TOOLU_CONFIG_DIR CODEX_HOME TYPESAFE_API_KEY
  HOOK="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/user-prompt-submit.sh"
  START="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/session-start.sh"
  PLUGIN_DIR="${HOOK%/hooks/user-prompt-submit.sh}"
}

teardown() {
  unset TOOLU_HOST_OVERRIDE PLUGIN_ROOT CODEX_HOME TOOLU_CONFIG_DIR TYPESAFE_API_KEY
  rm -rf "$TMP"
}

payload() {
  jq -nc --arg p "$1" '{hook_event_name: "UserPromptSubmit", prompt: $p}'
}

# The wrapper is published by SessionStart; a real session always runs it first.
publish() {
  bash "$START" <<<'{"source":"startup"}' >/dev/null
}

@test "user-prompt-submit: every task gets the mandate with the published wrapper path" {
  export TYPESAFE_API_KEY=local-test-key
  publish
  run bash "$HOOK" <<<"$(payload 'add pagination to the users endpoint')"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.hookSpecificOutput.hookEventName' <<<"$output")" = UserPromptSubmit ]
  context=$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")
  [[ "$context" == *"mandatory for this task"* ]]
  [[ "$context" == *"MUST call \"$CLAUDE_CONFIG_DIR/jev/jev.sh\""* ]]
  [[ "$context" == *"$PLUGIN_DIR/skills/jev/SKILL.md"* ]]
  [[ "$context" == *"one ask call"* ]]
  [[ "$context" == *"After initial exploration, identify useful judgments over supplied evidence"* ]]
  [[ "$context" == *"before the decision it informs"* ]]
  [[ "$context" == *"Reassess after new evidence, failed hypotheses, or changed requirements"* ]]
  [[ "$context" == *"Reuse unchanged evidence and questions rather than repeating calls"* ]]
  [[ "$context" != *"before acting"* ]]
  [[ "$context" != *"Before acting"* ]]
  [[ "$context" != *"at least one"* ]]
  [[ "$context" != *local-test-key* ]]
}

@test "user-prompt-submit: slash-command tasks are tasks too" {
  export TYPESAFE_API_KEY=local-test-key
  publish
  run bash "$HOOK" <<<"$(payload '/toolu:plan ship the export feature')"
  [ "$status" -eq 0 ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"mandatory for this task"* ]]
}

@test "user-prompt-submit: trivial confirmations are not tasks (silent)" {
  export TYPESAFE_API_KEY=local-test-key
  publish
  for p in yes 'ok' 'LGTM' 'go ahead' 'Continue.' '  y  ' 'thanks!'; do
    run bash "$HOOK" <<<"$(payload "$p")"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
  done
}

@test "user-prompt-submit: whitespace around a lone token is still trivial, but a multi-line task is not" {
  export TYPESAFE_API_KEY=local-test-key
  publish
  # Newlines and tabs around a single confirmation token: still a confirmation.
  run bash "$HOOK" <<<"$(payload $'\n\ty\t\n')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  # The same token inside a multi-line request: the anchored regex cannot match.
  run bash "$HOOK" <<<"$(payload $'ok\nnow add pagination to the users endpoint')"
  [ "$status" -eq 0 ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"mandatory for this task"* ]]
}

@test "user-prompt-submit: empty or missing prompt exits 0 silently" {
  export TYPESAFE_API_KEY=local-test-key
  publish
  run bash "$HOOK" <<<'{"hook_event_name":"UserPromptSubmit"}'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  run bash "$HOOK" <<<''
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "user-prompt-submit: missing TYPESAFE_API_KEY stays silent (SessionStart already reported it)" {
  publish
  run bash "$HOOK" <<<"$(payload 'add pagination to the users endpoint')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "user-prompt-submit: unpublished wrapper stays silent instead of naming a dead path" {
  export TYPESAFE_API_KEY=local-test-key
  run bash "$HOOK" <<<"$(payload 'add pagination to the users endpoint')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "user-prompt-submit: native Codex environment names the Codex wrapper path" {
  export PLUGIN_ROOT="$PLUGIN_DIR"
  export CODEX_HOME="$TMP/codex profile"
  export TYPESAFE_API_KEY=local-test-key
  publish
  run bash "$HOOK" <<<"$(payload 'triage the failing checks')"
  [ "$status" -eq 0 ]
  context=$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")
  [[ "$context" == *"\"$CODEX_HOME/jev/jev.sh\""* ]]
  [[ "$context" != *"$CLAUDE_CONFIG_DIR"* ]]
}

@test "user-prompt-submit: explicit config directory wins on both hosts" {
  export TOOLU_CONFIG_DIR="$TMP/custom profile"
  export TOOLU_HOST_OVERRIDE=codex
  export TYPESAFE_API_KEY=local-test-key
  publish
  run bash "$HOOK" <<<"$(payload 'triage the failing checks')"
  [ "$status" -eq 0 ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"\"$TOOLU_CONFIG_DIR/jev/jev.sh\""* ]]
}

@test "user-prompt-submit: paths with quotes and newlines remain JSON string data" {
  installed="$TMP/"$'plugin "cache"\nfolder'
  mkdir -p "$installed"
  cp -R "$PLUGIN_DIR/." "$installed/"
  export TOOLU_CONFIG_DIR="$TMP/"$'profile "quoted"\nfolder'
  export TYPESAFE_API_KEY=local-test-key
  bash "$installed/hooks/session-start.sh" <<<'{}' >/dev/null
  run bash "$installed/hooks/user-prompt-submit.sh" <<<"$(payload 'rank these approaches')"
  [ "$status" -eq 0 ]
  jq -e --arg wrapper "$TOOLU_CONFIG_DIR/jev/jev.sh" --arg skill_path "$installed/skills/jev/SKILL.md" '
    .hookSpecificOutput | .hookEventName == "UserPromptSubmit" and
    (.additionalContext | contains("\"" + $wrapper + "\"") and contains($skill_path + "."))' <<<"$output"
}

@test "user-prompt-submit: manifest command runs from an installed plugin path containing spaces" {
  installed="$TMP/plugin cache/jev"
  mkdir -p "$installed"
  cp -R "$PLUGIN_DIR/." "$installed/"
  export CLAUDE_PLUGIN_ROOT="$installed"
  export TYPESAFE_API_KEY=local-test-key
  start_command=$(jq -r '.hooks.SessionStart[0].hooks[0].command' "$installed/hooks/hooks.json")
  bash -c "$start_command" <<<'{"source":"startup"}' >/dev/null
  hook_command=$(jq -r '.hooks.UserPromptSubmit[0].hooks[0].command' "$installed/hooks/hooks.json")
  run bash -c "$hook_command" <<<"$(payload 'rank these approaches')"
  [ "$status" -eq 0 ]
  [[ "$(jq -r '.hookSpecificOutput.additionalContext' <<<"$output")" == *"$installed/skills/jev/SKILL.md"* ]]
}

# Fail-soft: a corrupted install where lib/ is missing must NOT break the turn.
@test "user-prompt-submit: shared lib missing -> exits 0, silent (fail-soft)" {
  fake="$TMP/fake-plugin/hooks"
  mkdir -p "$fake"
  cp "$HOOK" "$fake/user-prompt-submit.sh"
  export TYPESAFE_API_KEY=local-test-key
  run bash "$fake/user-prompt-submit.sh" <<<"$(payload 'rank these approaches')"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
