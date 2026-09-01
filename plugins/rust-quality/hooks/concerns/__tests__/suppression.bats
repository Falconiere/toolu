#!/usr/bin/env bats
# Covers the rust-quality 30-suppression concern: forbidden lint suppression
# (#[allow]/#[expect]/cfg_attr allow) is flagged, while an #[allow] mentioned
# only inside a line comment is not, and a test file's file-level #![allow]
# header is exempt while its per-item #[allow] is not. Drives the ASSEMBLED
# registry module assembled by register.sh.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)

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

_rust_project() {
  printf '[package]\nname = "x"\nversion = "0.1.0"\n' > Cargo.toml
  mkdir -p src
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "rust-quality: cfg_attr(allow) lint suppression is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
#[cfg_attr(test, allow(dead_code))]
fn helper() {}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
}

@test "rust-quality: #[allow] mentioned in a line comment is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/ok.rs <<'EOF'
// historically this used #[allow(dead_code)] elsewhere; removed now.
fn helper() -> u8 {
    1
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/ok.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden lint suppression"
}

# Test files carry a file-level allow header for the lints test code violates
# on purpose (unwrap/expect/panic). Only that INNER form is exempt, and only
# under tests/.
@test "rust-quality: file-level #![allow] header in a module-sibling test is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p src/api/tests
  cat > src/api/tests/stats.rs <<'EOF'
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn it_adds() {
    assert_eq!(1 + 1, 2);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/api/tests/stats.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden lint suppression"
}

@test "rust-quality: file-level #![allow] header in a crate-root integration test is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p tests
  cat > tests/integration.rs <<'EOF'
#![allow(clippy::unwrap_used)]

#[test]
fn it_runs() {
    assert!(true);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/integration.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden lint suppression"
}

@test "rust-quality: inner #![cfg_attr(..., allow(...))] in a test file is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p src/api/tests
  cat > src/api/tests/stats.rs <<'EOF'
#![cfg_attr(miri, allow(clippy::unwrap_used))]

#[test]
fn it_adds() {
    assert_eq!(1 + 1, 2);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/api/tests/stats.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden lint suppression"
}

@test "rust-quality: per-item #[allow] inside a test file IS still flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p src/api/tests
  cat > src/api/tests/stats.rs <<'EOF'
#[allow(dead_code)]
fn helper() {}

#[test]
fn it_adds() {
    assert_eq!(1 + 1, 2);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/api/tests/stats.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
  echo "$output" | grep -q "only the file-level #!\[allow(...)\] header is accepted"
}

@test "rust-quality: file-level #![allow] header outside tests/ IS still flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/lib.rs <<'EOF'
#![allow(clippy::unwrap_used)]

pub fn add(a: u8, b: u8) -> u8 {
    a + b
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/lib.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
  ! echo "$output" | grep -q "only the file-level"
}
