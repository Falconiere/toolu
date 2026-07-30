#!/usr/bin/env bats
# Covers the rust-quality 70-no-mocks concern: split scope zero-tolerance rules
# forbidding mock frameworks — mock *definitions* in src/ (#[automock],
# #[cfg_attr(..., automock)], mock! { ... }) and mock *imports* in test files
# (use mockall::...;, use faux::...;) — real data/fixtures only. Drives the
# ASSEMBLED registry module assembled by register.sh, same shape as the sibling
# concern suites (error-handling.bats, dispatch.bats).

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)

  # Assemble the registry module exactly as production does: point register.sh
  # at a temp CLAUDE_CONFIG_DIR and run it with no stdin.
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  HOOK="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/rust-quality@toolu__rust-quality.sh"

  TMP_PROJ="$TMP/proj"
  mkdir -p "$TMP_PROJ"
  cd "$TMP_PROJ"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Set up minimal Rust project so detect_rust passes.
_rust_project() {
  printf '[package]\nname = "x"\nversion = "0.1.0"\n' > Cargo.toml
  mkdir -p src tests
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "rust-quality no-mocks: src/ fixture with #[cfg_attr(test, automock)] fails the gate" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/service.rs <<'EOF'
#[cfg_attr(test, automock)]
trait Service {
    fn call(&self) -> u8;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/service.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks"
  echo "$output" | grep -q "QUALITY VIOLATION"
  jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -e '.violations | contains("no-mocks")' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
}

@test "rust-quality no-mocks: src/lib.rs with mock! { ... } fails the gate" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/lib.rs <<'EOF'
pub trait Foo {
    fn bar(&self) -> u8;
}

mock! {
    pub MyMock {}
    impl Foo for MyMock {
        fn bar(&self) -> u8;
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/lib.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks"
  echo "$output" | grep -q "mock! { ... }"
}

@test "rust-quality no-mocks: tests/integration_test.rs with use mockall::predicate::*; fails the gate" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > tests/integration_test.rs <<'EOF'
use mockall::predicate::*;

#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/integration_test.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks"
  echo "$output" | grep -q "mockall/faux import"
}

# use faux::...; import is the other half of the test-file scope.
@test "rust-quality no-mocks: tests/ fixture with use faux::...; fails the gate" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > tests/faux_test.rs <<'EOF'
use faux::create;

#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/faux_test.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks"
  echo "$output" | grep -q "mockall/faux import"
}

# A test file identified by *_test.rs basename outside tests/ is still in scope
# for the import rules (same path predicate as 20-tests.sh), independent of the
# tests/-placement check.
@test "rust-quality no-mocks: *_tests.rs basename outside tests/ is still scoped for mock imports" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > integration_tests.rs <<'EOF'
use mockall::automock;

#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/integration_tests.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks"
}

@test "rust-quality no-mocks: real-data test fixture (reads a real fixture file from tests/data/) passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  _rust_project
  mkdir -p tests/data
  echo "hello world" > tests/data/sample.txt
  cat > tests/real_data_test.rs <<'EOF'
use std::fs;

#[test]
fn reads_real_fixture() {
    let s = fs::read_to_string("tests/data/sample.txt").expect("fixture file must exist");
    assert_eq!(s.trim(), "hello world");
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/real_data_test.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}

@test "rust-quality no-mocks: lang.rust.noMocks=false lets a src/ automock fixture pass" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  _rust_project
  mkdir -p .claude
  echo '{"lang":{"rust":{"noMocks":false}}}' > .claude/toolu.config.json
  cat > src/service.rs <<'EOF'
#[cfg_attr(test, automock)]
trait Service {
    fn call(&self) -> u8;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/service.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}

@test "rust-quality no-mocks: lang.rust.noMocks=false lets a tests/ mockall-import fixture pass" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  _rust_project
  mkdir -p .claude
  echo '{"lang":{"rust":{"noMocks":false}}}' > .claude/toolu.config.json
  cat > tests/integration_test.rs <<'EOF'
use mockall::predicate::*;

#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/integration_test.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}

# ast-grep genuinely crashing (installed but broken) must still be a loud
# violation, not a silent pass — same contract as 60-error-handling.sh's crash
# test.
@test "rust-quality no-mocks: ast-grep crash while scanning for mocks is surfaced, not silent" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not on PATH"
  _rust_project
  mkdir -p "$TMP_PROJ/bin"
  printf '#!/bin/sh\necho boom >&2\nexit 2\n' > "$TMP_PROJ/bin/ast-grep"
  chmod +x "$TMP_PROJ/bin/ast-grep"
  cat > src/service.rs <<'EOF'
trait Service {
    fn call(&self) -> u8;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/service.rs"}}'
  PATH="$TMP_PROJ/bin:$PATH" tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "ast-grep failed while scanning"
  echo "$output" | grep -q "no-mocks rules could not be verified"
  echo "$output" | grep -q "boom"
}
