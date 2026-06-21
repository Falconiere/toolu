#!/usr/bin/env bats
# Regression tests for rust-scan.sh's error-surfacing contract (execution-review
# fix): a scan that cannot complete must FAIL LOUDLY (non-zero exit) and never
# emit a report that looks clean. The happy path carries an empty `errors` array.
# Real crates, no mocks.

setup() {
  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../../../.." && pwd)"
  SCAN="$REPO_ROOT/plugins/rust-quality/scripts/rust-scan.sh"
  command -v jq >/dev/null || skip "jq required"
}

# A real, structurally-clean single-crate workspace.
_make_clean_crate() {
  local dir="$1"
  mkdir -p "$dir/src"
  printf '[package]\nname = "p"\nversion = "0.1.0"\nedition = "2021"\n' > "$dir/Cargo.toml"
  printf '//! crate\n//!\n//! # Public API\n//!\n//! # Usage\npub fn a() {}\n' > "$dir/src/lib.rs"
}

@test "clean crate: exit 0 and errors is an empty array" {
  tmp="$(mktemp -d)"; _make_clean_crate "$tmp"
  run bash -c "'$SCAN' --path '$tmp' --json"
  [ "$status" -eq 0 ]
  run bash -c "'$SCAN' --path '$tmp' --json | jq -c '.errors'"
  [ "$status" -eq 0 ]
  [ "$output" = "[]" ]
  rm -rf "$tmp"
}

@test "non-Cargo target: non-zero exit, clear message, NOT a clean report" {
  tmp="$(mktemp -d)"   # empty dir, no Cargo.toml anywhere
  run bash -c "'$SCAN' --path '$tmp' --json"
  [ "$status" -ne 0 ]
  [[ "$output" != *'"summary"'* ]]   # must not emit a (clean-looking) report
  rm -rf "$tmp"
}

@test "broken lib resolution: scan fails loudly, never emits a clean report" {
  # A real misconfiguration (TOOLU_LIB_DIR points nowhere) must surface as a
  # non-zero exit, NOT a silent "total: 0" — the exact false-clean the fix kills.
  tmp="$(mktemp -d)"; _make_clean_crate "$tmp"
  run bash -c "TOOLU_LIB_DIR=/nonexistent/libs '$SCAN' --path '$tmp' --json"
  [ "$status" -ne 0 ]
  [[ "$output" != *'"total": 0'* ]]
  [[ "$output" != *'"total":0'* ]]
  rm -rf "$tmp"
}
