#!/usr/bin/env bats
# Covers the rust-quality 20-tests concern: test-file placement (test attrs must
# live under tests/), the inline #[cfg(test)] rule, and the no-false-positive
# guards (bench/wasm/not(test)/feature strings/doc-comment mentions). Drives the
# ASSEMBLED registry module assembled by register.sh.

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

@test "rust-quality: test file (#[test]) outside tests/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Rust test file outside tests/"
}

@test "rust-quality: integration test under tests/ is accepted" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p tests
  cat > tests/integration_test.rs <<'EOF'
#[test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/tests/integration_test.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "test file outside"
}

@test "rust-quality: bare #[rstest] test attribute outside tests/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
#[rstest]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Rust test file outside tests/"
}

# Regression: the test-attribute alternation hardcoded a few runtime prefixes,
# so `#[test_log::test]` (any `path::test`) and `#[test_case]` escaped the
# placement rule. The generalized `(path::)*test|test_case` now catches them.
@test "rust-quality: #[test_log::test] attribute outside tests/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
#[test_log::test]
fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Rust test file outside tests/"
}

@test "rust-quality: #[test_case(...)] attribute outside tests/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
#[test_case(1)]
fn it_works(x: u8) {
    assert_eq!(x, x);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Rust test file outside tests/"
}

# Guard: #[cfg(test)] must stay owned by the inline-cfg rule, not the widened
# test-attribute alternation (no `test`/`test_case` token follows `#[`).
@test "rust-quality: #[cfg(test)] is not caught by the widened test-attr rule" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/lib.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/lib.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
  ! echo "$output" | grep -q "Rust test file outside tests/"
}

# Regression: inline test config gated with all()/any() combinators
# (`#[cfg(all(test, feature = "x"))]`) must trip the inline-cfg(test) rule.
@test "rust-quality: inline #[cfg(all(test, ...))] is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/lib.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(all(test, feature = "unit"))]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/lib.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

# Guard the no-false-positive contract: `not(test)` is the OPPOSITE gate and a
# `feature = "test-..."` string must not be read as an inline test config.
@test "rust-quality: #[cfg(not(test))] and feature=\"test-x\" are NOT flagged as inline cfg(test)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/m.rs <<'EOF'
#[cfg(not(test))]
pub fn prod_only() -> u8 {
    1
}

#[cfg(feature = "test-utils")]
pub fn helper() -> u8 {
    2
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: canonical inline #[cfg(test)] mod produces one violation, not two" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/lib.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/lib.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
  ! echo "$output" | grep -q "Rust test file outside tests/"
}

@test "rust-quality: #[bench] and #[wasm_bindgen_test] do not trigger tests/ placement" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p benches
  cat > benches/speed.rs <<'EOF'
#[bench]
fn bench_it(b: &mut Bencher) {
    b.iter(|| 1 + 1);
}
EOF
  cat > src/wasm_checks.rs <<'EOF'
#[wasm_bindgen_test]
fn browser_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/benches/speed.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "test file outside"
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/wasm_checks.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "test file outside"
}

# Colocation fix: a bodyless `#[cfg(test)] mod tests;` wires an external
# module-sibling tests/ dir and must NOT trip the inline-cfg(test) rule.
@test "rust-quality: single-line bodyless #[cfg(test)] mod tests; passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)] mod tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: rustfmt two-line bodyless #[cfg(test)] / mod tests; passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
mod tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: two-line bodyless #[cfg(test)] / pub mod foo_tests; passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
pub mod foo_tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

# A crate that bans mod.rs can only wire a module-sibling test file with
# #[path], which puts an attribute between the cfg and the decl.
@test "rust-quality: #[path]-wired bodyless #[cfg(test)] mod tests; passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p src/api
  cat > src/api/stats.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
#[path = "tests/stats.rs"]
mod tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/api/stats.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: a run of attributes before the bodyless mod decl still passes" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
#[path = "tests/foo.rs"]
#[cfg_attr(miri, ignore)]
pub mod foo_tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: #[path]-wired #[cfg(test)] mod tests { with a body still fails" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
#[path = "tests/foo.rs"]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: single-line #[cfg(test)] mod tests { with a body still fails" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)] mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: two-line #[cfg(test)] / mod tests { with a body still fails" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

# The all()/any() combinator forms stay blocked unconditionally, even ahead of
# an otherwise-exempt bodyless mod declaration.
@test "rust-quality: #[cfg(all(test, ...))] followed by bodyless mod tests; still fails" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(all(test, feature = "unit"))]
mod tests;
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: #[cfg(test)] only in a doc comment does NOT suppress placement enforcement" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/foo.rs <<'EOF'
/// Example usage: annotate with #[cfg(test)] in your own crate.
#[tokio::test]
async fn it_works() {
    assert_eq!(1, 1);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/foo.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  # The doc-comment mention must not trip the inline-cfg(test) rule...
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
  # ...and placement enforcement must still fire on the real #[tokio::test].
  echo "$output" | grep -q "Rust test file outside tests/"
}

@test "rust-quality: cfg(all(test)) with no comma is still flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  printf '#[cfg(all(test))]\nmod tests {\n    fn t() {}\n}\n' > src/allnocomma.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/allnocomma.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: cfg(all(test , feature)) with space before comma is still flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  printf '#[cfg(all(test , feature = "x"))]\nmod tests;\n' > src/allspace.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/allspace.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Inline #\[cfg(test)\]"
}

@test "rust-quality: bodyless decl with trailing // comment is exempt" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  printf '#[cfg(test)]\nmod tests; // unit tests live in tests/\nfn real() {}\n' > src/trailing.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/trailing.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Inline #\[cfg(test)\]"
}
