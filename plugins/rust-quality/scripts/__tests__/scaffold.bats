#!/usr/bin/env bats
# Tests for the deterministic config scaffolder scripts/rust-scaffold.sh, driven
# against REAL Cargo workspaces built per-test (real .rs + Cargo.toml; no mocks).
# cargo + clippy are present and are used FOR REAL — AC-16 plants an .unwrap()
# and asserts `cargo clippy -- -D warnings` actually denies it (proving the
# scaffolded [workspace.lints] + per-member opt-in are LIVE, not inert).
#
# Coverage:
#   AC-10  merge-not-clobber: a pre-existing rustfmt.toml keeps max_width=120 and
#          a custom key, and gains the template keys it lacked; an existing
#          Cargo.toml keeps [package]/[dependencies] and GAINS [workspace.lints].
#   AC-16  propagation live: a real 2-member workspace — every member Cargo.toml
#          has `lints.workspace = true`, and a planted `let x = foo().unwrap();`
#          FAILS `cargo clippy --all-targets -- -D warnings`.
#   shapes single-crate / virtual manifest handled correctly.
#   idempotent: a second run leaves every file byte-identical to one run.

SCRIPTS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
SCAFFOLD="$SCRIPTS_DIR/rust-scaffold.sh"
TEMPLATES="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../templates" && pwd)"

setup() {
  REPO="$(mktemp -d "${BATS_TMPDIR:-/tmp}/scaffold.XXXXXX")"
}

teardown() {
  [ -n "${REPO:-}" ] && rm -rf "$REPO"
}

# --- fixture builders -------------------------------------------------------

# A real 2-member virtual workspace: crates/{core,api}, api depends on core.
make_workspace() {
  mkdir -p "$REPO/crates/core/src" "$REPO/crates/api/src"
  cat > "$REPO/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = ["crates/core", "crates/api"]
EOF
  cat > "$REPO/crates/core/Cargo.toml" <<'EOF'
[package]
name = "scaffold-core"
version = "0.1.0"
edition = "2021"
EOF
  cat > "$REPO/crates/api/Cargo.toml" <<'EOF'
[package]
name = "scaffold-api"
version = "0.1.0"
edition = "2021"

[dependencies]
scaffold-core = { path = "../core" }
EOF
  printf 'pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n' \
    > "$REPO/crates/core/src/lib.rs"
  printf 'pub fn run() -> u32 {\n    scaffold_core::add(1, 2)\n}\n' \
    > "$REPO/crates/api/src/lib.rs"
}

# A real single-crate package ([package] at root).
make_single_crate() {
  mkdir -p "$REPO/src"
  cat > "$REPO/Cargo.toml" <<'EOF'
[package]
name = "scaffold-solo"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF
  printf 'pub fn id(x: u32) -> u32 {\n    x\n}\n' > "$REPO/src/lib.rs"
}

run_scaffold() {
  run bash "$SCAFFOLD" --path "$REPO"
  [ "$status" -eq 0 ]
}

# --- AC-10: merge, never clobber -------------------------------------------

@test "AC-10: pre-existing rustfmt.toml keeps user keys and gains missing ones" {
  make_workspace
  # User's own config with a non-template value and a non-template key.
  cat > "$REPO/rustfmt.toml" <<'EOF'
# my style
max_width = 120
my_custom_key = "keepme"
EOF
  run_scaffold

  # User's value is PRESERVED (not reset to the template's 100).
  run grep -E '^max_width = 120$' "$REPO/rustfmt.toml"
  [ "$status" -eq 0 ]
  # The custom key survives.
  run grep -F 'my_custom_key = "keepme"' "$REPO/rustfmt.toml"
  [ "$status" -eq 0 ]
  # A template key the file lacked is ADDED (e.g. tab_spaces).
  run grep -E '^tab_spaces = 2$' "$REPO/rustfmt.toml"
  [ "$status" -eq 0 ]
  # The template's own max_width=100 was NOT appended (no clobber/dup).
  run grep -cE '^max_width' "$REPO/rustfmt.toml"
  [ "$output" -eq 1 ]
}

@test "AC-10: existing Cargo.toml keeps [package]/[dependencies] and gains [workspace.lints]" {
  make_single_crate
  run_scaffold

  # [package] and its keys are intact.
  run grep -E '^name = "scaffold-solo"$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
  run grep -E '^\[dependencies\]$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
  # It GAINED the lint tables.
  run grep -E '^\[workspace\.lints\.clippy\]$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
  run grep -E '^\[workspace\.lints\.rust\]$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
}

# --- AC-16: propagation is LIVE, not inert ---------------------------------

@test "AC-16: every member Cargo.toml gains lints.workspace = true" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  make_workspace
  run_scaffold

  for member in core api; do
    # Each member has a [lints] table opting into the workspace lints.
    run bash -c "grep -A1 -E '^\[lints\]$' '$REPO/crates/$member/Cargo.toml' | grep -E '^workspace = true$'"
    [ "$status" -eq 0 ]
  done
  # Root carries the [workspace.lints] block; members must NOT (only the opt-in).
  run grep -cE '^\[lints\]$' "$REPO/crates/core/Cargo.toml"
  [ "$output" -eq 1 ]
}

@test "AC-16: a planted .unwrap() is denied by cargo clippy -- -D warnings (lints live)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v cargo-clippy >/dev/null 2>&1 || skip "clippy not installed"
  make_workspace
  run_scaffold

  # Plant an unwrap on a NON-literal Option so the failing lint is unambiguously
  # clippy::unwrap_used (not unnecessary_literal_unwrap).
  cat > "$REPO/crates/api/src/lib.rs" <<'EOF'
//! api crate
pub fn pick(v: &[u32]) -> u32 {
    let first = v.first().copied();
    first.unwrap()
}
EOF

  # With the scaffolded deny + opt-in, clippy MUST fail.
  run bash -c "cd '$REPO' && cargo clippy --all-targets -- -D warnings 2>&1"
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" | grep -qE 'unwrap_used|used `unwrap'

  # Control: strip the member opt-in -> the very same code now PASSES, proving
  # the opt-in (not some default) is what makes the lint live.
  cat > "$REPO/crates/api/Cargo.toml" <<'EOF'
[package]
name = "scaffold-api"
version = "0.1.0"
edition = "2021"

[dependencies]
scaffold-core = { path = "../core" }
EOF
  run bash -c "cd '$REPO' && cargo clippy -p scaffold-api --all-targets -- -D warnings 2>&1"
  [ "$status" -eq 0 ]
}

# --- manifest shapes --------------------------------------------------------

@test "single-crate root gains BOTH [workspace.lints] and [lints] workspace = true" {
  make_single_crate
  run_scaffold
  run grep -E '^\[workspace\.lints\.clippy\]$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
  run bash -c "grep -A1 -E '^\[lints\]$' '$REPO/Cargo.toml' | grep -E '^workspace = true$'"
  [ "$status" -eq 0 ]
}

@test "virtual manifest with no members: root gains [workspace.lints], no [lints]" {
  cat > "$REPO/Cargo.toml" <<'EOF'
[workspace]
resolver = "2"
members = []
EOF
  run_scaffold
  run grep -E '^\[workspace\.lints\.clippy\]$' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]
  # No bare [lints] opt-in (there are no members to opt in).
  run grep -cE '^\[lints\]$' "$REPO/Cargo.toml"
  [ "$output" -eq 0 ]
}

# --- standalone-file write + hooks/CI --------------------------------------

@test "absent standalone files and hooks/CI are written from templates" {
  make_workspace
  run_scaffold
  [ -f "$REPO/rustfmt.toml" ]
  [ -f "$REPO/clippy.toml" ]
  [ -f "$REPO/deny.toml" ]
  [ -f "$REPO/lefthook.yml" ]
  [ -f "$REPO/.github/workflows/rust.yml" ]
  # The written clippy.toml matches the template byte-for-byte (pure copy).
  run diff "$TEMPLATES/clippy.toml" "$REPO/clippy.toml"
  [ "$status" -eq 0 ]
}

@test "existing lefthook.yml gains rust-check without dropping other hooks" {
  make_workspace
  cat > "$REPO/lefthook.yml" <<'EOF'
pre-commit:
  parallel: true
  commands:
    eslint:
      glob: "*.ts"
      run: npx eslint {staged_files}
EOF
  run_scaffold
  run grep -E '^[[:space:]]+rust-check:$' "$REPO/lefthook.yml"
  [ "$status" -eq 0 ]
  # The pre-existing hook survives.
  run grep -E '^[[:space:]]+eslint:$' "$REPO/lefthook.yml"
  [ "$status" -eq 0 ]
}

# --- idempotency ------------------------------------------------------------

@test "a second run is a byte-identical no-op (idempotent)" {
  make_workspace
  # Seed a partial pre-existing rustfmt.toml so the merge path runs too.
  printf '# mine\nmax_width = 120\n' > "$REPO/rustfmt.toml"
  cat > "$REPO/lefthook.yml" <<'EOF'
pre-commit:
  parallel: true
  commands:
    eslint:
      glob: "*.ts"
      run: npx eslint
EOF
  run_scaffold

  local snap
  snap="$(mktemp -d)"
  cp -R "$REPO/." "$snap/"

  run_scaffold
  run diff -r "$snap" "$REPO"
  [ "$status" -eq 0 ]
  rm -rf "$snap"
}
