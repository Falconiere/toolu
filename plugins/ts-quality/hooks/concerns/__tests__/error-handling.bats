#!/usr/bin/env bats
# Concern: error-handling — 78-error-ast (empty catch / silent .catch / swallow),
# 76-toast (manual try/catch+toast), 94-handler (await-without-try/catch advisory),
# plus the ast-grep-crash surfacing this concern drives. The 80-throw-literal
# rule + its gate-status persistence live in throw-literal.bats. Tests are ported
# VERBATIM from the deleted monolith per-rule suite; the only change is they drive
# the ASSEMBLED registry module (assembled in setup), not the deleted monolith.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)

  # Assemble the registry module exactly as production does, then point $HOOK at it.
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

# Set up a minimal TS project (tsconfig + bun lockfile) so detect_ts/detect_node_pm pass.
_ts_project() {
  echo '{}' > tsconfig.json
  echo '{"name":"x"}' > package.json
  touch bun.lock
  mkdir -p src
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "ts-quality: empty catch block is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  try {
    foo();
  } catch (e) {}
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Empty catch block"
}

@test "ts-quality: silent .catch(() => {}) is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  foo().catch(() => {});
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Silent promise rejection"
}

@test "ts-quality: clean error handling produces no error output" {
  _ts_project
  cat > src/good.ts <<'EOF'
export function good() {
  try {
    foo();
  } catch (e) {
    console.error(e);
    throw e;
  }
  foo().catch((err) => console.error(err));
  throw new Error("descriptive");
  throw new TypeError("typed");
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/good.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "QUALITY VIOLATION"
}

# Regression: the catch+toast awk used /catch\s*\(/ — BSD awk (macOS) has no \s,
# so `catch (` (with a space) never matched and the rule was silently dead.
@test "ts-quality: try/catch+toast in a component is flagged" {
  _ts_project
  mkdir -p src/components
  cat > src/components/save-button.tsx <<'EOF'
export function SaveButton() {
  try {
    save();
  } catch (error) {
    toast("save failed");
  }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/components/save-button.tsx"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Manual try/catch+toast"
}

# Regression: the await-without-handler advisory matched commented-out code
# both ways — `// await x()` raised it, `// try` suppressed it.
@test "ts-quality: commented-out await does not raise the handler advisory" {
  _ts_project
  cat > src/a.ts <<'EOF'
// await legacyCall() — kept for reference
export const flag = true;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/a.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "uses await with no try/catch"
}

@test "ts-quality: try only in a comment does not suppress the handler advisory" {
  _ts_project
  cat > src/a.ts <<'EOF'
// callers should try to handle this
export async function fetchIt(): Promise<number> {
  const r = await doFetch();
  return r;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/a.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "uses await with no try/catch"
}

@test "ts-quality: catch that returns null is flagged as swallow" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function f() {
  try {
    risky();
  } catch (e) {
    return null;
  }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "swallows the error"
}

@test "ts-quality: bare-return catch (no value) is flagged as swallow" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function f() {
  try {
    risky();
  } catch {
    return;
  }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "swallows the error"
}

# Regression: the rule used `try { $$$ } catch { return }` as an ast-grep
# pattern, but a bare `return` there is a WILDCARD over the optional argument —
# it matched `return []`, `return 42` and `return null` too, so every non-nullish
# fallback was reported as "returns a nullish value".
@test "ts-quality: catch returning a non-nullish value is NOT flagged as swallow" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/ok.ts <<'EOF'
/** Read a list, degrading to empty. */
export function readList(): string[] {
  try {
    return risky();
  } catch {
    return [];
  }
}
/** Count things, degrading to zero. */
export function count(): number {
  try {
    return risky();
  } catch (e) {
    return 0;
  }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/ok.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "swallows the error"
}

# The message body is fed to jq as a string; a literal backslash-n would come
# back out as "\\n" and render as one run-on line with a visible \n in it.
@test "ts-quality: violation excerpts are separated by real newlines, not a literal \\n" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  cat > src/bad.ts <<'EOF'
export function bad() {
  try {
    foo();
  } catch (e) {}
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/bad.ts"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext')
  # The headline and its excerpt must land on separate lines...
  echo "$ctx" | grep -q "^Empty catch block"
  echo "$ctx" | grep -qE "^${TMP}/src/bad.ts:[0-9]+:"
  # ...and no literal backslash-n may survive anywhere in the message.
  ! printf '%s' "$ctx" | grep -q '\\n'
}

@test "ts-quality: await with no try/catch emits a non-blocking advisory" {
  _ts_project
  cat > src/a.ts <<'EOF'
/** Fetch a thing. */
export async function f() {
  const r = await fetchThing("x");
  return r;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/a.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "uses await with no try/catch"
  ! echo "$output" | grep -q "QUALITY VIOLATION"
}

@test "ts-quality: await inside try/catch produces no error-handling advisory" {
  _ts_project
  cat > src/a.ts <<'EOF'
/** Fetch a thing safely. */
export async function f() {
  try {
    return await fetchThing("x");
  } catch (e) {
    throw new Error("fetch failed", { cause: e });
  }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/a.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "uses await with no try/catch"
}

@test "ts-quality: ast-grep crash is surfaced as a violation, not silent pass" {
  _ts_project
  # Stub ast-grep that crashes (exit > 1 with stderr noise).
  mkdir -p "$TMP/bin"
  printf '#!/bin/sh\necho boom >&2\nexit 2\n' > "$TMP/bin/ast-grep"
  chmod +x "$TMP/bin/ast-grep"
  cat > src/good.ts <<'EOF'
/** clean */
export function good() {
  return 1;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/good.ts"}}'
  PATH="$TMP/bin:$PATH" tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "ast-grep failed"
  echo "$output" | grep -q "boom"
  echo "$output" | grep -q "exit 2"
}
