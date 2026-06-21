#!/usr/bin/env bats
# Tests for the rust-quality scaffold templates. Real validation against the
# installed toolchain (cargo/rustfmt/clippy-driver/ast-grep present; taplo not),
# python3 for TOML/YAML parsing. No mocks — the actual template bytes are parsed.

TPL="${BATS_TEST_DIRNAME}/../templates"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Parse a TOML file with python3's tomllib; fail loudly on a parse error.
_parse_toml() {
  python3 - "$1" <<'PY'
import sys, tomllib
with open(sys.argv[1], "rb") as fh:
    tomllib.load(fh)
PY
}

# Parse a YAML file with PyYAML if available; else assert it's at least
# scannable for required keys (handled by the caller). Returns 2 when PyYAML
# is absent so the caller can fall back.
_parse_yaml() {
  python3 - "$1" <<'PY'
import sys
try:
    import yaml
except ModuleNotFoundError:
    sys.exit(2)
with open(sys.argv[1]) as fh:
    yaml.safe_load(fh)
PY
}

# ── rustfmt.toml ──────────────────────────────────────────────────────────

@test "rustfmt.toml: exists and rustfmt accepts it" {
  [ -f "$TPL/rustfmt.toml" ]
  printf 'fn main() {\n  println!("hi");\n}\n' > "$TMP/x.rs"
  run rustfmt --config-path "$TPL/rustfmt.toml" --check "$TMP/x.rs"
  [ "$status" -eq 0 ]
}

@test "rustfmt.toml: carries the expected key markers" {
  run grep -Fq 'edition = "2024"' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'max_width = 100' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'tab_spaces = 2' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'use_try_shorthand = true' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'reorder_imports = true' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
}

@test "rustfmt.toml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/rustfmt.toml"
  [ "$status" -eq 0 ]
}

# ── clippy.toml ───────────────────────────────────────────────────────────

@test "clippy.toml: is valid TOML" {
  [ -f "$TPL/clippy.toml" ]
  run _parse_toml "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
}

@test "clippy.toml: carries the threshold markers" {
  run grep -Fq 'too-many-lines-threshold' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'cognitive-complexity-threshold' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'too-many-arguments-threshold' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'type-complexity-threshold' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'msrv' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
}

@test "clippy.toml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/clippy.toml"
  [ "$status" -eq 0 ]
}

# ── clippy-lints.toml ─────────────────────────────────────────────────────

@test "clippy-lints.toml: is valid TOML" {
  [ -f "$TPL/clippy-lints.toml" ]
  run _parse_toml "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
}

@test "clippy-lints.toml: declares the workspace.lints tables to merge" {
  run grep -Fq '[workspace.lints.clippy]' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
  run grep -Fq '[workspace.lints.rust]' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
}

@test "clippy-lints.toml: carries the deny markers" {
  run grep -Fq 'unwrap_used' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'unsafe_code' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'panic = "deny"' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
}

@test "clippy-lints.toml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/clippy-lints.toml"
  [ "$status" -eq 0 ]
}

# ── deny.toml ─────────────────────────────────────────────────────────────

@test "deny.toml: is valid TOML" {
  [ -f "$TPL/deny.toml" ]
  run _parse_toml "$TPL/deny.toml"
  [ "$status" -eq 0 ]
}

@test "deny.toml: keeps the advisories section" {
  run grep -Fq '[advisories]' "$TPL/deny.toml"
  [ "$status" -eq 0 ]
  run grep -Fq 'ignore' "$TPL/deny.toml"
  [ "$status" -eq 0 ]
}

@test "deny.toml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/deny.toml"
  [ "$status" -eq 0 ]
}

# ── lefthook.rust.yml ─────────────────────────────────────────────────────

@test "lefthook.rust.yml: is valid YAML (or has the required structure)" {
  [ -f "$TPL/lefthook.rust.yml" ]
  run _parse_yaml "$TPL/lefthook.rust.yml"
  if [ "$status" -eq 2 ]; then
    # PyYAML absent — fall back to required-key structural assertions.
    run grep -q 'pre-commit:' "$TPL/lefthook.rust.yml"
    [ "$status" -eq 0 ]
    run grep -q 'rust-check:' "$TPL/lefthook.rust.yml"
    [ "$status" -eq 0 ]
  else
    [ "$status" -eq 0 ]
  fi
}

@test "lefthook.rust.yml: runs scan + fmt-check + clippy in the pre-commit hook" {
  run grep -Fq 'rust-scan.sh --staged' "$TPL/lefthook.rust.yml"
  [ "$status" -eq 0 ]
  run grep -Fq 'cargo fmt --all -- --check' "$TPL/lefthook.rust.yml"
  [ "$status" -eq 0 ]
  run grep -Fq 'cargo clippy --all-targets -- -D warnings' "$TPL/lefthook.rust.yml"
  [ "$status" -eq 0 ]
}

@test "lefthook.rust.yml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/lefthook.rust.yml"
  [ "$status" -eq 0 ]
}

# ── ci-rust.yml ───────────────────────────────────────────────────────────

@test "ci-rust.yml: is valid YAML (or has the required structure)" {
  [ -f "$TPL/ci-rust.yml" ]
  run _parse_yaml "$TPL/ci-rust.yml"
  if [ "$status" -eq 2 ]; then
    run grep -q '^jobs:' "$TPL/ci-rust.yml"
    [ "$status" -eq 0 ]
    run grep -q 'rust:' "$TPL/ci-rust.yml"
    [ "$status" -eq 0 ]
  else
    [ "$status" -eq 0 ]
  fi
}

@test "ci-rust.yml: runs fmt-check, clippy, nextest and the structural scan" {
  run grep -Fq 'cargo fmt --all -- --check' "$TPL/ci-rust.yml"
  [ "$status" -eq 0 ]
  run grep -Fq 'cargo clippy --all-targets -- -D warnings' "$TPL/ci-rust.yml"
  [ "$status" -eq 0 ]
  run grep -Fq 'nextest' "$TPL/ci-rust.yml"
  [ "$status" -eq 0 ]
  run grep -Fq 'rust-scan.sh' "$TPL/ci-rust.yml"
  [ "$status" -eq 0 ]
}

@test "ci-rust.yml: is leading-commented as a toolu scaffold template" {
  run grep -q 'toolu rust-quality scaffold template' "$TPL/ci-rust.yml"
  [ "$status" -eq 0 ]
}
