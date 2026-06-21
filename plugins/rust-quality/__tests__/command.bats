#!/usr/bin/env bats
# Tests for the rust-refactor command's deterministic scripts
# (spec docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md, AC-8).
#
# Real git repos and real crates — no mocks. Preflight behaviour (AC-8 + the
# Cargo-resolution guard) is exercised end to end; the apply pipeline gets a
# smoke check here (existence/executable/shellcheck/bad-args), its full
# behaviour is covered by the e2e step.

SCRIPTS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../scripts" && pwd)"
PREFLIGHT="$SCRIPTS_DIR/rust-refactor-preflight.sh"
APPLY="$SCRIPTS_DIR/rust-refactor-apply.sh"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Build a real, committed single-crate git repo at $1. Returns with a clean
# working tree on its default branch.
_make_crate_repo() {
  local repo="$1"
  mkdir -p "$repo/src"
  cat > "$repo/Cargo.toml" <<'TOML'
[package]
name = "fixture-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
TOML
  cat > "$repo/src/lib.rs" <<'RS'
//! Fixture crate.

pub fn answer() -> u32 {
  42
}
RS
  git -C "$repo" init -q
  git -C "$repo" config user.email "test@example.com"
  git -C "$repo" config user.name "Test"
  git -C "$repo" add -A
  git -C "$repo" commit -q -m "init"
}

# ── AC-8: preflight aborts on a dirty tree, succeeds + branches on a clean one ──

@test "AC-8: preflight on a dirty tree exits non-zero and makes no commit" {
  local repo="$TMP/dirty"
  _make_crate_repo "$repo"
  local before
  before="$(git -C "$repo" rev-parse HEAD)"

  # Dirty the tree with an uncommitted edit.
  printf '\npub fn extra() {}\n' >> "$repo/src/lib.rs"

  run "$PREFLIGHT" --path "$repo"
  [ "$status" -ne 0 ]
  [[ "$output" == *"dirty"* ]]

  # No commit was made and no refactor branch was created.
  [ "$(git -C "$repo" rev-parse HEAD)" = "$before" ]
  run git -C "$repo" show-ref --verify --quiet refs/heads/rust-quality/refactor
  [ "$status" -ne 0 ]
}

@test "AC-8: preflight on a clean tree exits 0 and creates rust-quality/refactor" {
  local repo="$TMP/clean"
  _make_crate_repo "$repo"

  run "$PREFLIGHT" --path "$repo"
  [ "$status" -eq 0 ]

  # The refactor branch exists and is checked out.
  run git -C "$repo" show-ref --verify --quiet refs/heads/rust-quality/refactor
  [ "$status" -eq 0 ]
  [ "$(git -C "$repo" rev-parse --abbrev-ref HEAD)" = "rust-quality/refactor" ]
}

@test "preflight is idempotent on a re-run (branch already exists)" {
  local repo="$TMP/again"
  _make_crate_repo "$repo"

  run "$PREFLIGHT" --path "$repo"
  [ "$status" -eq 0 ]
  # Second run: branch present + already checked out — still succeeds.
  run "$PREFLIGHT" --path "$repo"
  [ "$status" -eq 0 ]
  [ "$(git -C "$repo" rev-parse --abbrev-ref HEAD)" = "rust-quality/refactor" ]
}

# ── Non-Cargo target errors clearly ──

@test "preflight on a non-Cargo git dir errors clearly" {
  local repo="$TMP/nocargo"
  mkdir -p "$repo"
  git -C "$repo" init -q
  git -C "$repo" config user.email "test@example.com"
  git -C "$repo" config user.name "Test"
  printf 'hello\n' > "$repo/README.md"
  git -C "$repo" add -A
  git -C "$repo" commit -q -m "init"

  run "$PREFLIGHT" --path "$repo"
  [ "$status" -ne 0 ]
  [[ "$output" == *"Cargo workspace"* ]]
}

@test "preflight on a non-git dir errors clearly" {
  local repo="$TMP/notgit"
  mkdir -p "$repo/src"
  printf '[package]\nname="x"\nversion="0.1.0"\nedition="2021"\n' > "$repo/Cargo.toml"

  run "$PREFLIGHT" --path "$repo"
  [ "$status" -ne 0 ]
  [[ "$output" == *"git work tree"* ]]
}

@test "preflight requires --path" {
  run "$PREFLIGHT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"--path"* ]]
}

# ── apply smoke checks (full behaviour covered by the e2e step) ──

@test "apply: script exists and is executable" {
  [ -f "$APPLY" ]
  [ -x "$APPLY" ]
}

@test "apply: is shellcheck-clean (no suppressions)" {
  if ! command -v shellcheck >/dev/null 2>&1; then
    skip "shellcheck not installed"
  fi
  run shellcheck "$APPLY"
  [ "$status" -eq 0 ]
}

@test "preflight: is shellcheck-clean (no suppressions)" {
  if ! command -v shellcheck >/dev/null 2>&1; then
    skip "shellcheck not installed"
  fi
  run shellcheck "$PREFLIGHT"
  [ "$status" -eq 0 ]
}

@test "apply: --help exits 0 and prints usage" {
  run "$APPLY" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"--path"* ]]
}

@test "apply: a bad argument exits non-zero" {
  run "$APPLY" --bogus
  [ "$status" -ne 0 ]
}

@test "apply: missing --path exits non-zero" {
  run "$APPLY"
  [ "$status" -ne 0 ]
  [[ "$output" == *"--path"* ]]
}

@test "rust-refactor-apply aborts on a dirty working tree (clean-tree invariant)" {
  repo="$TMP/dirty"
  _make_crate_repo "$repo"
  printf 'stray' > "$repo/uncommitted.txt"
  run "$APPLY" --path "$repo"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not clean"* ]]
  [ -f "$repo/uncommitted.txt" ]
}
