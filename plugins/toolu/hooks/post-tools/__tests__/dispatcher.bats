#!/usr/bin/env bats
# Tests for the shared dispatcher (hooks/lib/dispatch.sh) under PostToolUse
# semantics, as used by hooks/post-tools/mod.sh.
#
# Guarantees:
#   - a module emitting top-level decision:"block" is authoritative: its
#     output is emitted immediately and later modules are skipped.
#   - permissionDecision is a PreToolUse-only concept and is IGNORED here
#     (PostToolUse hooks use decision:"block" + reason).
#   - advisory additionalContext from multiple modules merges into ONE object.
#   - top-level systemMessage advisories are merged into the final object.
#   - a module exiting non-zero (other than 2) does not kill the dispatcher.
#   - empty modules dir is a no-op.

setup() {
  TMP=$(mktemp -d)
  MODULES_DIR="$TMP/modules"
  mkdir -p "$MODULES_DIR"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../../lib/dispatch.sh
  . "$REPO_ROOT/hooks/lib/dispatch.sh"

  input='{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{"stdout":""}}'
  export input
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

write_module() {
  local name="$1"
  local body="$2"
  local path="$MODULES_DIR/${name}.sh"
  printf '%s\n' '#!/usr/bin/env bash' "$body" > "$path"
  chmod +x "$path"
}

@test "post dispatcher: decision:block short-circuits later modules" {
  write_module "a_block"    'jq -n "{decision:\"block\",reason:\"blocked-by-A\"}"'
  write_module "z_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"should-not-appear\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  count=$(echo "$output" | jq -s 'length')
  [ "$count" = "1" ]
  echo "$output" | jq -e '.decision == "block"'
  echo "$output" | jq -e '.reason | test("blocked-by-A")'
  ! echo "$output" | grep -q "should-not-appear"
}

@test "post dispatcher: advisory does NOT preempt a later block" {
  write_module "a_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"advisory-A\"}}"'
  write_module "z_block"    'jq -n "{decision:\"block\",reason:\"blocked-by-Z\"}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.decision == "block"'
  echo "$output" | jq -e '.reason | test("blocked-by-Z")'
}

@test "post dispatcher: permissionDecision deny is ignored (PreToolUse-only field)" {
  write_module "a_deny"     'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",permissionDecision:\"deny\",permissionDecisionReason:\"stale-pre-semantics\"}}"'
  write_module "b_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"still-runs\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "stale-pre-semantics"
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("still-runs")'
}

@test "post dispatcher: two advisory modules merge into ONE final output" {
  write_module "a_one" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"context-one\"}}"'
  write_module "b_two" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"context-two\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  count=$(echo "$output" | jq -s 'length')
  [ "$count" = "1" ]
  echo "$output" | jq -e '.hookSpecificOutput.hookEventName == "PostToolUse"'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("context-one")'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("context-two")'
}

@test "post normalized dispatcher: duplicate per-file advisories are deduplicated" {
  write_module "a_same" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"same-post-advisory\"}}"'
  patch=$'*** Begin Patch\n*** Add File: a.ts\n+x\n*** Add File: b.ts\n+y\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command},tool_response:"Done"}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext' | grep -c '^same-post-advisory$')" = 1 ]
}

@test "post normalized dispatcher: malformed apply_patch blocks normal result processing" {
  patch=$'*** Begin Patch\n*** Delete File: a.ts'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command},tool_response:"Done"}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.decision == "block" and (.reason | contains("parse apply_patch"))' >/dev/null
}

@test "post-tools entrypoint runs TypeScript and Rust concerns for every Codex patch path and clears deletions" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  project="$TMP/project"
  codex_home="$TMP/codex"
  mkdir -p "$project/src" "$codex_home/toolu"
  git -C "$project" init -q
  printf '%s\n' '{}' > "$project/tsconfig.json"
  : > "$project/bun.lock"
  printf '%s\n' '[package]' 'name="fixture"' 'version="0.1.0"' > "$project/Cargo.toml"
  git -C "$project" add tsconfig.json bun.lock Cargo.toml
  git -C "$project" -c user.email=t@t -c user.name=t commit -qm setup
  printf 'console.log("bad");\n' > "$project/src/bad.ts"
  printf '#[allow(dead_code)]\nfn bad() {}\n' > "$project/src/bad.rs"

  env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$codex_home" \
    bash "$REPO_ROOT/../ts-quality/hooks/register.sh" </dev/null
  env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$codex_home" \
    bash "$REPO_ROOT/../rust-quality/hooks/register.sh" </dev/null
  printf '%s\n' '{"version":1,"status":"ready","plugins":["rust-quality@toolu","ts-quality@toolu"]}' \
    > "$codex_home/toolu/codex-plugins.json"

  patch=$'*** Begin Patch\n*** Update File: src/bad.ts\n@@\n-a\n+b\n*** Update File: src/bad.rs\n@@\n-a\n+b\n*** End Patch'
  payload=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command},tool_response:"Done"}')
  run bash -c 'cd "$1" && env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$2" TOOLU_PROJECT_DIR="$1" bash "$3" <<<"$4"' \
    _ "$project" "$codex_home" "$REPO_ROOT/hooks/post-tools/mod.sh" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'Forbidden console.log'
  echo "$output" | grep -q 'Forbidden lint suppression'
  gate="$project/.codex/tmp/quality-gate-status.json"
  jq -e '.status == "failing" and (.entries["src/bad.ts"] != null) and (.entries["src/bad.rs"] != null)' "$gate" >/dev/null

  rm "$project/src/bad.ts" "$project/src/bad.rs"
  delete_patch=$'*** Begin Patch\n*** Delete File: src/bad.ts\n*** Delete File: src/bad.rs\n*** End Patch'
  delete_payload=$(jq -cn --arg command "$delete_patch" '{tool_name:"apply_patch",tool_input:{command:$command},tool_response:"Done"}')
  run bash -c 'cd "$1" && env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$2" TOOLU_PROJECT_DIR="$1" bash "$3" <<<"$4"' \
    _ "$project" "$codex_home" "$REPO_ROOT/hooks/post-tools/mod.sh" "$delete_payload"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing" and ((.entries // {}) | length == 0)' "$gate" >/dev/null
}

@test "post dispatcher: systemMessage advisories are merged into the final output" {
  write_module "a_msg" 'jq -n "{systemMessage:\"message-one\"}"'
  write_module "b_ctx" 'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"context-two\"},systemMessage:\"message-two\"}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  count=$(echo "$output" | jq -s 'length')
  [ "$count" = "1" ]
  echo "$output" | jq -e '.systemMessage | test("message-one")'
  echo "$output" | jq -e '.systemMessage | test("message-two")'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext == "context-two"'
}

@test "post dispatcher: module exiting non-zero does not kill the dispatcher" {
  write_module "a_broken" 'echo "boom" >&2; exit 1'
  write_module "b_good"   'jq -n "{hookSpecificOutput:{hookEventName:\"PostToolUse\",additionalContext:\"good-context\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "a_broken.sh"
  echo "$output" | grep -q "good-context"
}

@test "post dispatcher: module exit 2 propagates as block (status 2, stderr forwarded)" {
  write_module "a_block" 'echo "post-hard-block" >&2; exit 2'

  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 2 ]
  echo "$output" | grep -q "post-hard-block"
}

@test "post dispatcher: empty modules dir is a no-op" {
  run toolu_dispatch_modules "$MODULES_DIR" "PostToolUse"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
