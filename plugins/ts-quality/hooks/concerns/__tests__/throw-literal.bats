#!/usr/bin/env bats
# Concern: throw-literal — 80-throw-literal (throw of Error-with-no-message /
# string / numeric / null; comment carve-outs) plus the gate-status persistence
# that a throw violation drives (file written, per-file clear does not clobber).
# Split out of error-handling.bats to keep each file under the size cap. Ported
# VERBATIM from the deleted monolith per-rule suite; only change: drive the
# ASSEMBLED registry module.

TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  HOOK="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/ts-quality@toolu__ts-quality.sh"

  TMP_PROJ="$TMP/proj"
  mkdir -p "$TMP_PROJ"
  cd "$TMP_PROJ"
  TMP="$TMP_PROJ"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
  [ -n "${CLAUDE_CONFIG_DIR:-}" ] && [ -d "$CLAUDE_CONFIG_DIR" ] && rm -rf "$CLAUDE_CONFIG_DIR"
}

_ts_project() {
  echo '{}' > tsconfig.json
  echo '{"name":"x"}' > package.json
  touch bun.lock
  mkdir -p src
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "ts-quality: throw new Error() with no message is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  throw new Error();
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no message"
}

@test "ts-quality: throw of string literal is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  throw "bad";
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "string literal"
}

@test "ts-quality: throw of numeric literal is flagged" {
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  throw 42;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "non-Error literal"
}

@test "ts-quality: throw of null is flagged" {
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  throw null;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "non-Error literal"
}

# Regression: the PostToolUse matcher includes MultiEdit, but the file-path
# extraction only ran for Write/Edit — a MultiEdit on a .ts file (with
# CLAUDE_FILE_PATHS unset) silently skipped all quality checks.
@test "ts-quality: MultiEdit extracts file path and flags violations (regression)" {
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() { throw 42; }
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=MultiEdit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "non-Error literal"
}

@test "ts-quality: throw literal inside // inline comment is NOT flagged" {
  _ts_project
  cat > src/good.ts <<'EOF'
export function good() {
  const x = 1; // we used to throw 5 here
  return x;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/good.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "non-Error literal"
}

@test "ts-quality: throw literal inside /* */ block comment is NOT flagged" {
  _ts_project
  cat > src/good.ts <<'EOF'
export function good() {
  /* avoid: throw 42 — use Error */
  return 1;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/good.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "non-Error literal"
}

@test "ts-quality: gate-status file is written on violation" {
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() { throw 42; }
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -f "$TMP/.claude/tmp/quality-gate-status.json" ]
  jq -e '.status == "failing"' "$TMP/.claude/tmp/quality-gate-status.json"
  jq -e '.source == "ts-quality-hook"' "$TMP/.claude/tmp/quality-gate-status.json"
}

@test "ts-quality: clearing one file does not clobber another file's failure" {
  _ts_project
  printf 'console.log("a");\n' > src/a.ts
  printf 'console.log("b");\n' > src/b.ts
  payload_a='{"tool_input":{"file_path":"'"$TMP"'/src/a.ts"}}'
  payload_b='{"tool_input":{"file_path":"'"$TMP"'/src/b.ts"}}'
  GATE="$TMP/.claude/tmp/quality-gate-status.json"

  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"

  # b.ts goes clean — a.ts's violation must survive and keep the gate failing.
  printf 'console.info("b");\n' > src/b.ts
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e --arg f "$TMP/src/a.ts" '.entries[$f]' "$GATE"

  # a.ts goes clean too — now the gate may pass.
  printf 'console.info("a");\n' > src/a.ts
  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE"
}

@test "ts-quality: deleting a failing file clears its gate entry" {
  _ts_project
  printf 'console.log("bad");\n' > "$TMP/src/bad.ts"
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$TMP/.claude/tmp/quality-gate-status.json" >/dev/null

  rm "$TMP/src/bad.ts"
  TOOLU_EDIT_OPERATION=delete tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$TMP/.claude/tmp/quality-gate-status.json" >/dev/null
}
