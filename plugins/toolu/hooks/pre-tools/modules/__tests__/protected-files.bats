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
}

teardown() {
  unset TOOLU_SETTINGS_DIR
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

run_hook() {
  local file="$1"
  local payload
  payload=$(jq -n --arg p "$file" '{tool_name:"Edit",tool_input:{file_path:$p}}')
  tool_name=Edit input="$payload" run bash "$HOOK" <<<"$payload"
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

@test "protected-files: blocks .env (bare basename)" {
  run_hook ".env"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "protected-files: blocks hooks/lib/detect.sh (path glob)" {
  run_hook "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

# Regression: MultiEdit must be subject to the protected-files deny. The hook
# previously skipped any tool that was not Edit/Write, letting MultiEdit bypass
# protection on the same paths — a security-equivalent hole.
@test "protected-files: blocks MultiEdit on hooks/lib/detect.sh (path glob)" {
  run_hook_multiedit "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
}

@test "protected-files: allows src/foo.ts" {
  run_hook "src/foo.ts"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# Regression: Claude's Edit/Write sends absolute paths. Without prefix
# normalization, [[ /abs/path == hooks/lib/** ]] is false and trusted-script
# protection silently no-ops.
@test "protected-files: blocks ABSOLUTE path under hooks/lib/** (real-world payload)" {
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/hooks/lib"
  touch "$REPO/hooks/lib/detect.sh"
  ( cd "$REPO" && run_hook "$REPO/hooks/lib/detect.sh" >/dev/null 2>&1 )
  # Re-run inside the repo so detect_project_root resolves correctly.
  cd "$REPO"
  run_hook "$REPO/hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  rm -rf "$REPO"
}

@test "protected-files: blocks ABSOLUTE path under hooks/**/*.sh" {
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/hooks/post-tools/modules"
  touch "$REPO/hooks/post-tools/modules/rust-quality.sh"
  cd "$REPO"
  run_hook "$REPO/hooks/post-tools/modules/rust-quality.sh"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
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
