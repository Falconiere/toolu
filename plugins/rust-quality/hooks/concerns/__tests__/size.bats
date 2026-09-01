#!/usr/bin/env bats
# Covers the three rust-quality size concerns merged into one suite:
#   10-size-file  — file line-count limit (+ approximate-size fallback)
#   50-size-fn    — per-fn length via brace-depth counter (visibility/qualifier
#                   prefixes, inner control flow, unbalanced string braces)
#   55-size-impl  — per-impl block size via brace-depth counter
# Drives the ASSEMBLED registry module assembled by register.sh.

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

# --- 10-size-file ---

@test "rust-quality: project config lowers maxFileLines, flags a file the default would not" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFileLines":10}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  : > src/big.rs
  for i in $(seq 1 15); do echo "pub const V$i: u32 = $i;" >> src/big.rs; done
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/big.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "exceeds 10-line limit"
}

@test "rust-quality: over-limit message flags approximate size on unterminated /*" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFileLines":2}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  # A string containing /* flips count_code_lines into the raw-count fallback;
  # has_unterminated_block detects the unbalanced /* and the message says so.
  printf '%s\n' 'let s = "/*";' 'let a = 1;' 'let b = 2;' 'let c = 3;' > src/m.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "exceeds 2-line limit"
  echo "$output" | grep -q "size approximated"
}

# --- 50-size-fn ---

@test "rust-quality: pub(crate) const fn is subject to the fn-length limit" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":3}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
pub(crate) const fn big() -> u8 {
    let a = 1;
    let b = 2;
    let c = 3;
    a + b + c
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}

# Regression: the fn-end marker used /^[[:space:]]*\}/, so the close of the
# FIRST inner if/else/match block ended the measured range — any long fn with
# control flow was measured as a few lines and never flagged.
@test "rust-quality: long fn with inner if/else is measured to its real close (regression)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":5}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/long.rs <<'EOF'
fn long_with_branches(x: u8) -> u8 {
    let mut acc = 0;
    if x > 0 {
        acc += 1;
    } else {
        acc += 2;
    }
    acc += 3;
    acc += 4;
    acc
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/long.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}

# Regression: with the col-0 fn-end marker, the LAST method in an impl was
# measured to the impl's close — trailing non-fn items inflated its length and
# the report named the wrong fn. The brace-depth counter measures each method
# to its own close.
@test "rust-quality: short impl method followed by other items is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":5}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
pub struct Foo;
impl Foo {
    fn short(&self) -> u8 {
        1
    }
    const A: u8 = 1;
    const B: u8 = 2;
    const C: u8 = 3;
    const D: u8 = 4;
    const E: u8 = 5;
    const F: u8 = 6;
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Function too long"
}

@test "rust-quality: long method inside an impl IS flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":5}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
pub struct Foo;
impl Foo {
    fn long_method(&self, x: u8) -> u8 {
        let mut acc = 0;
        if x > 0 {
            acc += 1;
        } else {
            acc += 2;
        }
        acc += 3;
        acc
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}

# Regression: a lone brace inside a "..." string or '{' char literal skewed the
# brace-depth counter, leaving the fn unmeasured (silently skipped).
@test "rust-quality: long fn with unbalanced brace in a string is still measured" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":5}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
fn unbalanced_string(x: u8) -> u8 {
    let open = "{";
    let close = '{';
    if x > 0 {
        return 1;
    }
    let a = 2;
    let b = 3;
    a + b
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}

# Regression: pub(in path) restricted visibility was rejected by the old
# pub(\([a-z]+\))? prefix, so a long pub(in …) fn slipped the length gate.
@test "rust-quality: pub(in path) fn is subject to the fn-length limit" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":3}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
pub(in crate::foo) fn big() -> u8 {
    let a = 1;
    let b = 2;
    let c = 3;
    a + b + c
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}

# --- 55-size-impl ---

@test "rust-quality: oversized impl block is flagged (brace-depth)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxImplLines":6}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  : > src/m.rs
  echo 'pub struct Foo;' >> src/m.rs
  echo 'impl Foo {' >> src/m.rs
  for i in $(seq 1 8); do echo "    pub const V$i: u8 = $i;" >> src/m.rs; done
  echo '}' >> src/m.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Impl block too large"
}

@test "rust-quality: small impl with an inner-control-flow method is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxImplLines":20}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  cat > src/m.rs <<'EOF'
pub struct Foo;
impl Foo {
    fn pick(&self, x: u8) -> u8 {
        if x > 0 {
            1
        } else {
            2
        }
    }
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/m.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Impl block too large"
}

# --- test-file exemption (50-size-fn / 55-size-impl) ---
#
# A table-driven test is one long fn by nature, and the toolu-conventions Rust
# test header prescribes `clippy::too_many_lines` for exactly that reason. A
# crate that bans mod.rs wires its colocated tests as src/<mod>/tests/<name>.rs
# — under src/, but test code all the same — so the size rules key on the file
# being test code, not on the directory.

@test "rust-quality: long fn in a COLOCATED test under src/ is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxFnLines":3}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  mkdir -p src/store/tests
  cat > src/store/tests/big.rs <<'EOF'
#[test]
fn table_driven() {
    let a = 1;
    let b = 2;
    let c = 3;
    assert_eq!(a + b + c, 6);
}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/store/tests/big.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Function too long"
}

@test "rust-quality: oversized impl in a COLOCATED test under src/ is NOT flagged" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude"
  echo '{"lang":{"rust":{"maxImplLines":6}}}' > "$TMP_PROJ/.claude/toolu.config.json"
  mkdir -p src/store/tests
  : > src/store/tests/big.rs
  echo '#[test]' >> src/store/tests/big.rs
  echo 'fn anchor() { assert!(true); }' >> src/store/tests/big.rs
  echo 'struct Fixture;' >> src/store/tests/big.rs
  echo 'impl Fixture {' >> src/store/tests/big.rs
  for i in $(seq 1 8); do echo "    const V$i: u8 = $i;" >> src/store/tests/big.rs; done
  echo '}' >> src/store/tests/big.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/store/tests/big.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Impl block too large"
}
