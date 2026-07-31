#!/usr/bin/env bats
# Concern: no-mocks — 85-no-mocks (jest.mock/vi.mock/jest.fn/vi.fn/sinon.$M calls
# and ts-mockito imports forbidden in test files; tests must exercise real
# data/services). Also covers the paired change to 20-tests.sh: the
# __tests__/mocks/ subdir is no longer whitelisted — a whitelisted mocks/
# layout would contradict a blocking no-mock gate. Drives the ASSEMBLED
# registry module, same fixture shape as the sibling concern suites.

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

@test "ts-quality: vi.mock in a test file is flagged by no-mocks" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
import { dep } from "../dep";
vi.mock("../dep");
test("uses dep", () => {
  expect(dep).toBeDefined();
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Mocked test double"
  echo "$output" | grep -q "QUALITY VIOLATION"
  jq -e '.status == "failing"' "$TMP/.claude/tmp/quality-gate-status.json"
  jq -e '.violations | contains("Mocked test double")' "$TMP/.claude/tmp/quality-gate-status.json"
}

@test "ts-quality: jest.fn in a test file is flagged by no-mocks" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
const handler = jest.fn();
test("calls handler", () => {
  handler();
  expect(handler).toBeDefined();
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Mocked test double"
}

@test "ts-quality: real-data test fixture (reads a real JSON fixture) passes no-mocks" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__/fixtures
  echo '{"name":"Ada"}' > src/__tests__/fixtures/user.json
  cat > src/__tests__/foo.test.ts <<'EOF'
import { readFileSync } from "node:fs";
import { join } from "node:path";

test("loads a real user fixture", () => {
  const raw = readFileSync(join(__dirname, "fixtures/user.json"), "utf8");
  const user = JSON.parse(raw);
  expect(user.name).toBe("Ada");
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Mocked test double"
}

@test "ts-quality: import from ts-mockito is flagged by no-mocks" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
import { mock, instance } from "ts-mockito";

test("uses ts-mockito", () => {
  const m = mock<{ x: number }>();
  expect(instance(m)).toBeDefined();
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Import from ts-mockito"
}

@test "ts-quality: lang.ts.noMocks=false lets a vi.mock fixture pass" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  mkdir -p "$TMP/.claude"
  echo '{"lang":{"ts":{"noMocks":false}}}' > "$TMP/.claude/toolu.config.json"
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
import { dep } from "../dep";
vi.mock("../dep");
test("uses dep", () => {
  expect(dep).toBeDefined();
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Mocked test double"
}

# Regression: __tests__/mocks/ used to be whitelisted by 20-tests.sh (see
# 20-tests.sh's allowed-subdir list) — a whitelisted mocks/ layout contradicts
# a blocking no-mock gate, so the entry was removed. A test file placed there
# must now fail 20-tests.sh's nested-subdirectory check.
@test "ts-quality: __tests__/mocks/ subdir now fails 20-tests (whitelist removed)" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__/mocks
  cat > src/__tests__/mocks/handlers.test.ts <<'EOF'
test("noop", () => {
  expect(true).toBe(true);
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/mocks/handlers.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Test nested in __tests__/ subdirectory"
}

# Other allowed subdirs are unaffected by the mocks removal.
@test "ts-quality: __tests__/fixtures/ subdir still allowed by 20-tests" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__/fixtures
  cat > src/__tests__/fixtures/handlers.test.ts <<'EOF'
test("noop", () => {
  expect(true).toBe(true);
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/fixtures/handlers.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Test nested in __tests__/ subdirectory"
}

@test "ts-quality: e2e specs are exempt from no-mocks" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  mkdir -p src/app/e2e
  cat > src/app/e2e/login.spec.ts <<'EOF'
vi.mock("../session");
test("logs in", () => {
  expect(true).toBe(true);
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/app/e2e/login.spec.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Mocked test double"
}

@test "ts-quality: sinon.stub in a test file is flagged by no-mocks" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
import { dep } from "../dep";
const stub = sinon.stub(dep, "toString");
test("uses stub", () => {
  expect(stub).toBeDefined();
});
EOF
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Mocked test double"
}

# ast-grep genuinely crashing (installed but broken) must still be a loud
# violation, not a silent pass — same contract as 78-error-ast.sh's crash test.
@test "ts-quality: ast-grep crash while scanning for mocks is surfaced, not silent" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
test("clean", () => {
  expect(true).toBe(true);
});
EOF
  mkdir -p "$TMP/bin"
  printf '#!/bin/sh\necho boom >&2\nexit 2\n' > "$TMP/bin/ast-grep"
  chmod +x "$TMP/bin/ast-grep"
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  PATH="$TMP/bin:$PATH" tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "ast-grep failed while scanning"
  echo "$output" | grep -q "boom"
}

# ast-grep exiting 0 with genuinely empty stdout (no output at all, not even
# "[]") must also be surfaced as a tool failure, not silently treated as a
# clean "no hits" scan — jq itself exits 0 on empty input too, so without
# this check the two are indistinguishable.
@test "ts-quality: ast-grep exit 0 with empty stdout while scanning for mocks is surfaced, not silent" {
  _ts_project
  echo 'export const dep = 1;' > src/dep.ts
  mkdir -p src/__tests__
  cat > src/__tests__/foo.test.ts <<'EOF'
test("clean", () => {
  expect(true).toBe(true);
});
EOF
  mkdir -p "$TMP/bin"
  printf '#!/bin/sh\nexit 0\n' > "$TMP/bin/ast-grep"
  chmod +x "$TMP/bin/ast-grep"
  payload='{"tool_input":{"file_path":"'"$TMP"'/src/__tests__/foo.test.ts"}}'
  PATH="$TMP/bin:$PATH" tool_name=Write input="$payload" PROJECT_ROOT="$TMP" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "ast-grep failed while scanning"
  echo "$output" | grep -q "empty output"
}
