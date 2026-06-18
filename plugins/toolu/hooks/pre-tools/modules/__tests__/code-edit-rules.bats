#!/usr/bin/env bats
# Tests for hooks/pre-tools/modules/code-edit-rules.sh

HOOK="${BATS_TEST_DIRNAME}/../code-edit-rules.sh"

setup() {
  TMP=$(mktemp -d)
  export TOOLU_SETTINGS_DIR="$TMP/settings"
  mkdir -p "$TOOLU_SETTINGS_DIR"
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

# MultiEdit carries .tool_input.file_path and is in the PreToolUse matcher, so
# rule reminders must surface for MultiEdit too.
run_hook_multiedit() {
  local file="$1"
  local payload
  payload=$(jq -n --arg p "$file" \
    '{tool_name:"MultiEdit",tool_input:{file_path:$p,edits:[{old_string:"a",new_string:"b"}]}}')
  tool_name=MultiEdit input="$payload" run bash "$HOOK" <<<"$payload"
}

@test "code-edit-rules: empty rules → no-op" {
  printf '%s\n' '{"rules":[]}' > "$TOOLU_SETTINGS_DIR/code-edit-rules.json"
  run_hook "/abs/src/foo.rs"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "code-edit-rules: missing file → no-op" {
  # No code-edit-rules.json in TOOLU_SETTINGS_DIR.
  run_hook "/abs/src/foo.rs"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "code-edit-rules: matching glob surfaces docs" {
  cat > "$TOOLU_SETTINGS_DIR/code-edit-rules.json" <<'JSON'
{ "rules": [ { "match": "*.rs", "docs": ["rust.md"] } ] }
JSON
  run_hook "/abs/src/foo.rs"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("rust.md")'
}

# Regression: MultiEdit was skipped (tool != Edit/Write), dropping rule
# reminders. MultiEdit on a code file must emit the same reminder as Edit.
@test "code-edit-rules: MultiEdit on .rs file surfaces docs" {
  cat > "$TOOLU_SETTINGS_DIR/code-edit-rules.json" <<'JSON'
{ "rules": [ { "match": "*.rs", "docs": ["Rust: zero compiler/clippy warnings"] } ] }
JSON
  run_hook_multiedit "/abs/src/foo.rs"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("Rust: zero compiler/clippy warnings")'
}

# Regression: absolute path under a git repo must be normalized to repo-relative
# so repo-relative match globs like "src/**/*.rs" fire.
@test "code-edit-rules: ABSOLUTE path under repo normalizes to repo-relative for glob match" {
  cat > "$TOOLU_SETTINGS_DIR/code-edit-rules.json" <<'JSON'
{ "rules": [ { "match": "src/**/*.rs", "docs": ["rust.md"] } ] }
JSON
  # Resolve symlinks so the path we feed matches what `git rev-parse --show-toplevel`
  # returns inside the repo (macOS /var → /private/var symlink would otherwise
  # foil the prefix strip).
  REPO=$(cd "$(mktemp -d)" && pwd -P)
  ( cd "$REPO" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init )
  mkdir -p "$REPO/src/foo"
  touch "$REPO/src/foo/bar.rs"
  cd "$REPO"
  run_hook "$REPO/src/foo/bar.rs"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("rust.md")'
  rm -rf "$REPO"
}
