#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/protected-files.sh

HOOK="${BATS_TEST_DIRNAME}/../protected-files.sh"

setup() {
  TMP=$(mktemp -d)
  export TOOLU_SETTINGS_DIR="$TMP/settings"
  mkdir -p "$TOOLU_SETTINGS_DIR"
  cat > "$TOOLU_SETTINGS_DIR/protected-files.txt" <<'TXT'
.env
.env.*
**/secrets/**
hooks/lib/**
hooks/**/*.sh
TXT

  # Isolated config layers so a real ~/.claude or repo config cannot decide
  # what mode these cases resolve. No gates key written: the cases below
  # assert the SHIPPED default (ask) unless they write one themselves.
  export HOME="$TMP/home"
  export TOOLU_PROJECT_DIR="$TMP/project"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE
  mkdir -p "$HOME/.claude" "$TOOLU_PROJECT_DIR/.claude"
}

teardown() {
  unset TOOLU_SETTINGS_DIR TOOLU_PROJECT_DIR CLAUDE_PROJECT_DIR
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

run_hook() {
  local file="$1"
  local payload
  payload=$(jq -n --arg p "$file" '{tool_name:"Edit",tool_input:{file_path:$p}}')
  tool_name=Edit input="$payload" run bash "$HOOK" <<<"$payload"
}

# Issue #176: Bash/Shell writes against a protected path had no path-shaped
# tool_input to check and sailed through the Edit/Write-only deny.
run_hook_bash() {
  local command="$1"
  local payload
  payload=$(jq -n --arg c "$command" '{tool_name:"Bash",tool_input:{command:$c}}')
  tool_name=Bash input="$payload" run bash "$HOOK" <<<"$payload"
}

# MultiEdit carries .tool_input.file_path (plus an edits[] array). The
# PreToolUse matcher includes MultiEdit, so a protected path edited via
# MultiEdit must be denied exactly like Edit/Write — not silently bypassed.
run_hook_multiedit() {
  local file="$1"
  local payload
  payload=$(jq -n --arg p "$file" \
    '{tool_name:"MultiEdit",tool_input:{file_path:$p,edits:[{old_string:"a",new_string:"b"}]}}')
  tool_name=MultiEdit input="$payload" run bash "$HOOK" <<<"$payload"
}

@test "protected-files: asks on .env (bare basename)" {
  run_hook ".env"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: asks on hooks/lib/detect.sh (path glob)" {
  run_hook "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

# Regression: MultiEdit must be subject to the protected-files deny. The hook
# previously skipped any tool that was not Edit/Write, letting MultiEdit bypass
# protection on the same paths — a security-equivalent hole.
@test "protected-files: asks on MultiEdit on hooks/lib/detect.sh (path glob)" {
  run_hook_multiedit "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: allows src/foo.ts" {
  run_hook "src/foo.ts"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: Claude's Edit/Write sends absolute paths. Without prefix
# normalization, [[ /abs/path == hooks/lib/** ]] is false and trusted-script
# protection silently no-ops.
@test "protected-files: asks on ABSOLUTE path under hooks/lib/** (real-world payload)" {
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/hooks/lib"
  touch "$REPO/hooks/lib/detect.sh"
  ( cd "$REPO" && run_hook "$REPO/hooks/lib/detect.sh" >/dev/null 2>&1 )
  # Re-run inside the repo so detect_project_root resolves correctly.
  cd "$REPO"
  run_hook "$REPO/hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
  rm -rf "$REPO"
}

@test "protected-files: asks on ABSOLUTE path under hooks/**/*.sh" {
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/hooks/post-tools/modules"
  touch "$REPO/hooks/post-tools/modules/rust-quality.sh"
  cd "$REPO"
  run_hook "$REPO/hooks/post-tools/modules/rust-quality.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
  rm -rf "$REPO"
}

@test "protected-files: allows ABSOLUTE path outside protected globs" {
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/src"
  touch "$REPO/src/foo.ts"
  cd "$REPO"
  run_hook "$REPO/src/foo.ts"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  rm -rf "$REPO"
}

@test "protected-files: the ask prompt carries the loud guardrail banner" {
  run_hook ".env"
  [ "$status" -eq 0 ]
  local reason
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  # The banner is the whole point: this must not read like a routine "allow?".
  [[ "$reason" == *"SECURITY GUARDRAIL — OVERRIDE REQUESTED"* ]]
  # It names the path, says WHY that path is guarded, and scopes the approval.
  [[ "$reason" == *".env"* ]]
  [[ "$reason" == *"WHY THIS IS GUARDED"* ]]
  [[ "$reason" == *"secrets file"* ]]
  [[ "$reason" == *"THIS ONE CALL"* ]]
}

@test "protected-files: the why-line is specific to the KIND of protected path" {
  run_hook "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  local reason
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  # A hook file is guarded for a different reason than a secrets file, and the
  # prompt has to say which -- a generic "it is protected" gets waved through.
  [[ "$reason" == *"enforcement code"* ]]
  [[ "$reason" != *"secrets file"* ]]
}

@test "protected-files: gates.protectedFiles.mode=block restores the hard deny" {
  printf '%s' '{"version":1,"gates":{"protectedFiles":{"mode":"block"}}}' \
    > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run_hook ".env"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "protected-files: gates.protectedFiles.mode=off disables the check" {
  printf '%s' '{"version":1,"gates":{"protectedFiles":{"mode":"off"}}}' \
    > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run_hook ".env"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "protected-files: gates.protectedFiles.mode=advise warns without stopping" {
  printf '%s' '{"version":1,"gates":{"protectedFiles":{"mode":"advise"}}}' \
    > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run_hook ".env"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]
  [[ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')" == *"NOT stopped"* ]]
}

@test "protected-files: on a host that cannot prompt, the guardrail BLOCKS (never advises)" {
  # Fail closed: no human to ask means the answer is no. An advise here would
  # hand an agent silent .env access on exactly the hosts nobody is watching.
  mkdir -p "$CLAUDE_PROJECT_DIR/.codex"
  printf '%s' '{"version":1}' > "$CLAUDE_PROJECT_DIR/.codex/toolu.config.json"
  local payload
  payload=$(jq -n '{tool_name:"Edit",tool_input:{file_path:".env"}}')
  tool_name=Edit input="$payload" TOOLU_HOST_OVERRIDE=codex \
    TOOLU_PROJECT_DIR="$CLAUDE_PROJECT_DIR" run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

# ── Bash/Shell coverage (issue #176) ────────────────────────────────────────

@test "protected-files: Bash redirect '>' onto .env asks" {
  run_hook_bash 'echo hi > .env'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: Bash 'sed -i' on .env.example asks" {
  run_hook_bash "sed -i 's/a/b/' .env.example"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: Bash python3 -c open(FILE, 'w') is denied -- the issue #176 repro" {
  run_hook_bash "python3 -c \"open('.env', 'w').write(x)\""
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: Bash 'tee' onto a protected path asks" {
  run_hook_bash 'echo hi | tee .env'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: Bash prompt says it is a WRITE and names the target" {
  run_hook_bash 'echo hi > .env'
  [ "$status" -eq 0 ]
  local reason
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"SECURITY GUARDRAIL — OVERRIDE REQUESTED"* ]]
  [[ "$reason" == *"would WRITE to .env"* ]]
  [[ "$reason" == *"secrets file"* ]]
}

@test "protected-files: Bash write hidden in an unquoted heredoc substitution asks" {
  # bash expands $(...) inside <<EOF, so this really writes .env.
  run_hook_bash "$(printf '%s\n' 'cat <<EOF' '$(echo hi > .env)' 'EOF')"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}

@test "protected-files: Bash second open() behind a benign first is still denied" {
  run_hook_bash "python3 -c \"open('ok.txt','w'); open('.env','w')\""
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | contains(".env")'
}

@test "protected-files: Bash command writing an unprotected path is allowed" {
  run_hook_bash 'echo hi > /tmp/scratch.txt'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "protected-files: Bash command reading .env.example is allowed" {
  run_hook_bash 'cat .env.example'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "protected-files: Shell tool_name is covered the same as Bash" {
  local payload
  payload=$(jq -n --arg c 'echo hi > .env' '{tool_name:"Shell",tool_input:{command:$c}}')
  tool_name=Shell input="$payload" run bash "$HOOK" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"'
}
