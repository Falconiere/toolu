#!/usr/bin/env bats
# Tests for pl_doc_field (hooks/lib/plan-ledger-parse.sh): extract an inline-bold
# **Field:** value from a packed doc header line. Real fixture docs, no mocks.

bats_require_minimum_version 1.5.0

setup() {
  TMP=$(mktemp -d)
  # shellcheck source=../plan-ledger-parse.sh
  . "${BATS_TEST_DIRNAME}/../plan-ledger-parse.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# A header line that packs four fields with varying whitespace between them, plus
# a title line above and prose below — proving the extractor anchors on the bold
# key, not line position.
_write_header_doc() {
  cat > "$1" <<'EOF'
# Some Plan — Plan

**Date:** 2026-06-17   **Status:** Draft   **Spec:** docs/toolu/specs/x.md   **Topic:** Harden the thing

## Context

Prose below the header.
EOF
}

@test "pl_doc_field: extracts a mid-line field, trimmed, stopping at the next bold key" {
  doc="$TMP/plan.md"
  _write_header_doc "$doc"
  run pl_doc_field "$doc" Status
  [ "$status" -eq 0 ]
  [ "$output" = "Draft" ]
}

@test "pl_doc_field: extracts a field followed by a multi-space gap then next key" {
  doc="$TMP/plan.md"
  _write_header_doc "$doc"
  run pl_doc_field "$doc" Spec
  [ "$status" -eq 0 ]
  [ "$output" = "docs/toolu/specs/x.md" ]
}

@test "pl_doc_field: trailing (last-on-line) field runs to EOL" {
  doc="$TMP/plan.md"
  _write_header_doc "$doc"
  run pl_doc_field "$doc" Topic
  [ "$status" -eq 0 ]
  [ "$output" = "Harden the thing" ]
}

@test "pl_doc_field: missing field -> empty string, return 0" {
  doc="$TMP/plan.md"
  _write_header_doc "$doc"
  run pl_doc_field "$doc" Author
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_doc_field: single-space-separated fields still stop at the next bold key" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
# Tight Header — Plan

**Status:** Approved **Spec:** none **Topic:** terse
EOF
  run pl_doc_field "$doc" Status
  [ "$status" -eq 0 ]
  [ "$output" = "Approved" ]
}

@test "pl_doc_field: absent doc -> empty string, return 0 (never errors)" {
  run pl_doc_field "$TMP/nope.md" Status
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
