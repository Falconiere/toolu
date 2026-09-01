#!/usr/bin/env bats
# Covers the rust-quality 60-error-handling concern: the zero-tolerance
# ast-grep rules for .unwrap()/.expect()/panic!/todo!/unimplemented!/unreachable!
# in src/, clean Result code, ast-grep crash surfacing, and the gate write.
# Drives the ASSEMBLED registry module assembled by register.sh.

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
  mkdir -p src
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "rust-quality: .unwrap() in src/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
fn main() {
    let x = some_result().unwrap();
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q ".unwrap()"
}

@test "rust-quality: .expect() in src/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
fn main() {
    let y = thing.expect("nope");
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q ".expect()"
}

@test "rust-quality: panic!/todo!/unimplemented! in src/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
fn main() {
    panic!("explode");
    todo!();
    unimplemented!();
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "panic!/todo!/unimplemented!"
}

@test "rust-quality: clean Result-based code produces no error output" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/good.rs <<'EOF'
fn main() -> Result<(), std::io::Error> {
    let _ = std::fs::read_to_string("x")?;
    Ok(())
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/good.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "QUALITY VIOLATION"
}

@test "rust-quality: ast-grep crash is surfaced as a violation, not silent pass" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  # Stub ast-grep that crashes (exit > 1 with stderr noise).
  mkdir -p "$TMP_PROJ/bin"
  printf '#!/bin/sh\necho boom >&2\nexit 2\n' > "$TMP_PROJ/bin/ast-grep"
  chmod +x "$TMP_PROJ/bin/ast-grep"
  cat > src/good.rs <<'EOF'
fn helper() {}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/good.rs"}}'
  PATH="$TMP_PROJ/bin:$PATH" tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "ast-grep failed"
  # The captured stderr detail must be surfaced so the agent sees WHAT broke.
  echo "$output" | grep -q "boom"
  echo "$output" | grep -q "exit 2"
}

@test "rust-quality: gate-status file is written on violation" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
fn main() { panic!(); }
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -f "$TMP_PROJ/.claude/tmp/quality-gate-status.json" ]
  jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -e '.source == "rust-quality-hook"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
}

@test "rust-quality: unreachable! in src/ is flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
fn f(x: u8) -> u8 {
    match x {
        0 => 1,
        _ => unreachable!("never"),
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "unreachable!"
}

@test "rust-quality: .unwrap() in a COLOCATED test under src/ is not flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  # A crate that bans mod.rs wires its module-sibling tests as
  # src/<mod>/tests/<name>.rs — under src/, but test code all the same.
  mkdir -p src/store/tests
  cat > src/store/tests/migrate.rs <<'EOF'
#[test]
fn migration_applies() {
    let conn = open().unwrap();
    assert_eq!(conn.version(), 14);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/store/tests/migrate.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -qF ".unwrap()"
}

@test "rust-quality: .unwrap() in a NON-test file under a src/.../tests/ path is still flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  # The exemption keys on the file being test code (filename or a #[test]
  # attribute), NOT on the directory alone — a helper with neither stays
  # production code even sitting in a tests/ dir.
  mkdir -p src/store/tests
  cat > src/store/tests/helper.rs <<'EOF'
pub fn open_or_die() -> Conn {
    connect().unwrap()
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/store/tests/helper.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q ".unwrap()"
}
