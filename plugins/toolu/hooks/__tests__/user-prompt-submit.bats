#!/usr/bin/env bats
# Tests for hooks/user-prompt-submit.sh — must remain project-agnostic.

HOOK="${BATS_TEST_DIRNAME}/../user-prompt-submit.sh"

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

build_input() {
  jq -n --arg p "$1" '{prompt: $p}'
}

@test "user-prompt-submit: runs without context.sh present" {
  payload=$(build_input "implement a new feature for the dashboard")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
}

@test "user-prompt-submit: sources .claude/context.sh when present" {
  mkdir -p .claude
  cat > .claude/context.sh <<'CTX'
#!/usr/bin/env bash
echo "CUSTOM_PROJECT_CONTEXT_MARKER"
CTX
  chmod +x .claude/context.sh
  payload=$(build_input "implement a new feature")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "CUSTOM_PROJECT_CONTEXT_MARKER"
}

@test "user-prompt-submit: output does not mention yamless or routo" {
  payload=$(build_input "refactor the engine module")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -qE 'yamless|routo|/Volumes/Projects/(routo|yamless)'
}

@test "user-prompt-submit: trivial prompt exits without output" {
  payload=$(build_input "yes")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "user-prompt-submit: no recall hint when prompt does not invite recall" {
  # A formatting tweak names no recall word and no task verb, so the recall
  # branch must stay silent. (Task verbs like 'implement'/'add' now DO invite
  # recall — see the two cases below.)
  payload=$(build_input "reformat this block")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'Recall first'
  ! echo "$output" | grep -q 'comemory search'
}

@test "user-prompt-submit: task verb 'implement' invites recall" {
  payload=$(build_input "implement a new dashboard widget")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  # The recall branch fires for ordinary task verbs now: the recall line when
  # comemory is available, a WARN when it is absent (as under the sanitized test
  # PATH). Either proves the verb entered the branch — mirrors the architecture
  # case above.
  echo "$output" | grep -qE 'comemory|Recall first|WARN'
}

@test "user-prompt-submit: task verb 'add' invites recall" {
  payload=$(build_input "add a retry to the fetch helper")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | grep -qE 'comemory|Recall first|WARN'
}

@test "user-prompt-submit: recall hint NOT emitted for 'paste' (substring of past)" {
  payload=$(build_input "paste this snippet at the top of the file")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'Recall first'
  ! echo "$output" | grep -q 'comemory search'
}

@test "user-prompt-submit: recall hint emitted when prompt mentions architecture" {
  payload=$(build_input "explain the architecture of the auth module")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  # Recall hint only when comemory is available; warning when missing.
  # Either way the response must mention comemory or a warn, not be silent.
  echo "$output" | grep -qE 'comemory|Recall first|WARN'
}

@test "user-prompt-submit: emits at most one intent hint per prompt" {
  # Prompt matches multiple regex categories (fix + test + delete + review).
  payload=$(build_input "fix and remove the failing test review the code")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  # Count hint markers; should be 0 or 1, never more.
  count=0
  echo "$ctx" | grep -q 'Fix in code'       && count=$((count+1))
  echo "$ctx" | grep -q 'real-world data'   && count=$((count+1))
  echo "$ctx" | grep -q 'Verify no deps'    && count=$((count+1))
  echo "$ctx" | grep -q 'Review: forbidden' && count=$((count+1))
  echo "$ctx" | grep -q 'Structural pattern' && count=$((count+1))
  echo "$ctx" | grep -q 'Rename:'           && count=$((count+1))
  [ "$count" -le 1 ]
}

@test "user-prompt-submit: 'implement' does NOT trigger structural pattern hint" {
  # Regression: `impl` was an alternation token and matched as substring of
  # `implement`. Dropped from the structural alternation and word-boundary
  # wrapped so this prompt now produces NO intent hint at all.
  payload=$(build_input "implement a new feature")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'Structural pattern'
  ! echo "$ctx" | grep -q 'ast-grep'
}

@test "user-prompt-submit: 'remove' does NOT trigger rename hint" {
  # Regression: `move` matched as substring of `remove`. Word boundary now
  # gates this; `remove` falls through to the delete branch.
  payload=$(build_input "remove the old helper")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'Rename:'
  echo "$ctx" | grep -q 'Verify no deps'
}

@test "user-prompt-submit: 'fast' does NOT trigger structural pattern hint" {
  # Regression: `ast` matched as substring of `fast`/`past`/`last`. Dropped
  # entirely from the structural alternation.
  payload=$(build_input "make this loop faster")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'Structural pattern'
}

@test "user-prompt-submit: 'dropdown' does NOT trigger delete hint" {
  # Regression: `drop` matched as substring of `dropdown`. Dropped from the
  # delete alternation.
  payload=$(build_input "create a dropdown menu component")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'Verify no deps'
}

@test "user-prompt-submit: 'preview' does NOT trigger review hint" {
  # Regression: `review` matched as substring of `preview`. Word boundary
  # now gates this.
  payload=$(build_input "preview the changes")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'Review: forbidden'
}

write_failing_gate() {
  mkdir -p .claude/tmp
  printf '%s\n' '{"status":"failing","reason":"forced"}' > .claude/tmp/quality-gate-status.json
}

@test "user-prompt-submit: 'prefix' does NOT suppress the failing-gate hint" {
  # Regression: suppression matched `fix` as a substring, so any prompt
  # containing `prefix` silently dropped the quality-gate warning.
  write_failing_gate
  payload=$(build_input "add a prefix to the logger module")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'Quality gate failing'
}

@test "user-prompt-submit: 'fix the gate' suppresses the failing-gate hint" {
  write_failing_gate
  payload=$(build_input "fix the gate so the build passes")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'Quality gate failing'
}

@test "user-prompt-submit: omits branch/git context (moved to SessionStart)" {
  payload=$(build_input "implement a new feature")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -qE 'Branch: .*(clean|uncommitted)'
}

@test "user-prompt-submit: orchestration nudge fires on a broad/multi-step prompt" {
  payload=$(build_input "refactor the auth module across the codebase")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'orchestrator skill'
}

@test "user-prompt-submit: orchestration nudge is independent of the intent hint" {
  # "fix" -> intent hint; "across" -> orchestration nudge. Both must appear.
  payload=$(build_input "fix the login bug across all services")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'Fix in code'
  echo "$ctx" | grep -q 'orchestrator skill'
}

@test "user-prompt-submit: no orchestration nudge for a narrow single-file prompt" {
  payload=$(build_input "add a button to the settings page")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'orchestrator skill'
}

@test "user-prompt-submit: 'auditor' does NOT trigger the orchestration nudge" {
  # Word-boundary regression: 'audit' must not match inside 'auditor'.
  payload=$(build_input "add an auditor name field to the form")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'orchestrator skill'
}

@test "user-prompt-submit: research nudge fires on an external-knowledge prompt" {
  payload=$(build_input "look up the latest React Compiler docs")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'research-agent subagent'
}

@test "user-prompt-submit: research nudge is independent of the intent hint" {
  # "fix" -> intent hint; "release notes" -> research nudge. Both must appear.
  payload=$(build_input "fix the parser per the latest release notes")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'Fix in code'
  echo "$ctx" | grep -q 'research-agent subagent'
}

@test "user-prompt-submit: no research nudge for a codebase 'where is' prompt" {
  payload=$(build_input "where is the auth middleware defined")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'research-agent subagent'
}

@test "user-prompt-submit: no research nudge for a codebase 'how does' prompt" {
  payload=$(build_input "how does the parser work in this repo")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'research-agent subagent'
}

@test "user-prompt-submit: research nudge suppressed when agents.research-agent is false" {
  # Project config in the git-init'd TMP root toggles the agent off.
  mkdir -p .claude
  cat > .claude/toolu.config.json <<'CFG'
{ "agents": { "research-agent": false } }
CFG
  payload=$(build_input "look up the latest React Compiler docs")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'research-agent subagent'
}

@test "user-prompt-submit: jira nudge fires on the word 'jira'" {
  payload=$(build_input "create a task at jira")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'use the `jira` skill'
  echo "$ctx" | grep -q 'NOT the Atlassian MCP'
}

@test "user-prompt-submit: jira nudge fires on a pasted atlassian.net/browse link" {
  payload=$(build_input "https://acme.atlassian.net/browse/ABC-123 please move it to done")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'use the `jira` skill'
}

@test "user-prompt-submit: jira nudge fires on an issue key with a context word" {
  payload=$(build_input "fix the bug in ABC-123 ticket")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'use the `jira` skill'
}

@test "user-prompt-submit: jira nudge is independent of the intent hint" {
  # "fix" -> intent hint; "ABC-123 ticket" -> jira nudge. Both must appear.
  payload=$(build_input "fix the bug in ABC-123 ticket")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  echo "$ctx" | grep -q 'Fix in code'
  echo "$ctx" | grep -q 'use the `jira` skill'
}

@test "user-prompt-submit: NO jira nudge for key-shaped tokens without a context word" {
  # UTF-8 and GPT-4 match the issue-key shape but carry no jira-context word,
  # so the gate must keep the nudge silent.
  payload=$(build_input "decode the UTF-8 string and bump to GPT-4")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'jira` skill'
}

@test "user-prompt-submit: NO jira nudge for an unrelated prompt" {
  payload=$(build_input "list my files in the current directory")
  run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext // ""')
  ! echo "$ctx" | grep -q 'jira` skill'
}

@test "user-prompt-submit: recall hint points at the published wrapper path, not a bare comemory.sh" {
  # Regression: the hint said `comemory.sh search "<topic>"`, but the wrapper is
  # NOT on PATH — register.sh publishes it at $CLAUDE_CONFIG_DIR/comemory/
  # comemory.sh. The bare form dies with command-not-found, whose 0-byte stdout
  # the agent misreads as "no memories". Stub the binary so the state resolves
  # to `available` and the recall branch (not the WARN branch) fires.
  local stub="$BATS_TEST_TMPDIR/bin"
  mkdir -p "$stub"
  printf '#!/bin/sh\nexit 0\n' > "$stub/comemory"
  chmod +x "$stub/comemory"
  payload=$(build_input "implement a new dashboard widget")
  run env PATH="$stub:$PATH" bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'Recall first'
  # The published path template, left unexpanded for the agent's own shell.
  echo "$output" | grep -qF '${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh'
  # ...and never the bare command form that is not on PATH.
  ! echo "$output" | grep -qF '`comemory.sh search'
}
