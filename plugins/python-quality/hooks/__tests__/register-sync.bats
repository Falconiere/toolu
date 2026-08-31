#!/usr/bin/env bats
# Drift guard: rust-quality, ts-quality, and python-quality register.sh are
# intentionally the SAME assembler, differing only in the SPEC=/OUT= identity
# lines and the plugin-name comments. We deliberately did NOT extract a shared
# lib (they run at SessionStart cross-plugin, as detached blobs — no stable
# shared path), so this test enforces that the LOGIC stays byte-identical by
# hand. If it fails, port the change to ALL affected register.sh, not just one.

ROOT="${BATS_TEST_DIRNAME}/../../../.."   # hooks/__tests__ -> hooks -> python-quality -> plugins -> repo root

# Strip identity lines (SPEC=/OUT=) and all comments, leaving only executable logic.
_norm() { grep -vE '^(SPEC=|OUT=|[[:space:]]*#|#)' "$1"; }

@test "register.sh: python-quality and rust-quality logic is identical modulo SPEC/OUT/comments" {
  python="$ROOT/plugins/python-quality/hooks/register.sh"
  rust="$ROOT/plugins/rust-quality/hooks/register.sh"
  [ -f "$python" ]
  [ -f "$rust" ]
  diff <(_norm "$python") <(_norm "$rust")
}

@test "register.sh: python-quality and ts-quality logic is identical modulo SPEC/OUT/comments" {
  python="$ROOT/plugins/python-quality/hooks/register.sh"
  ts="$ROOT/plugins/ts-quality/hooks/register.sh"
  [ -f "$python" ]
  [ -f "$ts" ]
  diff <(_norm "$python") <(_norm "$ts")
}
