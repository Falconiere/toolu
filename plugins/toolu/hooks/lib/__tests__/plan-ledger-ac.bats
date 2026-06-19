#!/usr/bin/env bats
# Tests for hooks/lib/plan-ledger-parse.sh — AC parsing + ac_refs resolution.
# Real fixture spec/plan docs in a temp dir. No mocks.

bats_require_minimum_version 1.5.0

setup() {
  TMP=$(mktemp -d)
  # shellcheck source=../plan-ledger-parse.sh
  . "${BATS_TEST_DIRNAME}/../plan-ledger-parse.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# A spec with AC-1 and AC-2 under the canonical heading, plus an AC-line in a
# DIFFERENT section that must not be picked up, and a duplicate AC-1 to dedupe.
_write_spec_two() {
  cat > "$1" <<'EOF'
# Some Spec

## Problem

A problem statement that mentions **AC-1:** in prose before the section.

## Acceptance criteria

- **AC-1:** First criterion proven by real input.
- **AC-2:** Second criterion.

## Open Questions

- **AC-9:** This is NOT an acceptance criterion (different section).
EOF
}

# A plan whose two steps reference AC-1 and AC-2 (all resolve against _write_spec_two).
_write_plan_resolving() {
  cat > "$1" <<'EOF'
# Some Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First", "check": "true", "ac_refs": ["AC-1"] },
  { "id": "s2", "title": "Second", "check": "true", "ac_refs": ["AC-2"] }
]
```
EOF
}

# A plan with a dangling ref: AC-1 resolves, AC-9 does not (not under the heading).
_write_plan_dangling() {
  cat > "$1" <<'EOF'
# Some Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First", "check": "true", "ac_refs": ["AC-1", "AC-9"] }
]
```
EOF
}

@test "pl_parse_acs: extracts AC ids under the heading, ignores other sections (AC-5)" {
  spec="$TMP/spec.md"
  _write_spec_two "$spec"

  run pl_parse_acs "$spec"
  [ "$status" -eq 0 ]
  # Exactly AC-1 and AC-2, in document order. The prose AC-1 before the section
  # and the AC-9 in Open Questions are excluded.
  [ "$output" = "$(printf 'AC-1\nAC-2')" ]
}

@test "pl_parse_acs: duplicate AC ids are deduped, first-seen order kept (AC-5)" {
  spec="$TMP/spec.md"
  cat > "$spec" <<'EOF'
# Spec

## Acceptance criteria

- **AC-1:** first.
- **AC-2:** second.
- **AC-1:** a duplicate that must be deduped.
- **AC-5:** non-contiguous id is allowed (gaps fine).
EOF

  run pl_parse_acs "$spec"
  [ "$status" -eq 0 ]
  [ "$output" = "$(printf 'AC-1\nAC-2\nAC-5')" ]
}

@test "pl_parse_acs: spec with no '## Acceptance criteria' section -> empty, rc 0 (AC-10)" {
  spec="$TMP/spec.md"
  cat > "$spec" <<'EOF'
# Spec

## Problem

Nothing acceptance-related here.
EOF

  run pl_parse_acs "$spec"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_parse_acs: missing spec file -> empty, rc 0 (no error)" {
  run pl_parse_acs "$TMP/does-not-exist.md"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_check_ac_refs: all refs resolve -> nothing printed, rc 0 (AC-5)" {
  spec="$TMP/spec.md"; plan="$TMP/plan.md"
  _write_spec_two "$spec"
  _write_plan_resolving "$plan"

  run pl_check_ac_refs "$plan" "$spec"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_check_ac_refs: dangling ref is flagged, rc non-zero (AC-5)" {
  spec="$TMP/spec.md"; plan="$TMP/plan.md"
  _write_spec_two "$spec"
  _write_plan_dangling "$plan"

  run pl_check_ac_refs "$plan" "$spec"
  [ "$status" -ne 0 ]
  [ "$output" = "AC-9" ]
}

@test "pl_check_ac_refs: spec='none' -> passes, no dangling (AC-10)" {
  plan="$TMP/plan.md"
  _write_plan_dangling "$plan"

  run pl_check_ac_refs "$plan" "none"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_check_ac_refs: spec omitted -> passes, no dangling (AC-10)" {
  plan="$TMP/plan.md"
  _write_plan_dangling "$plan"

  run pl_check_ac_refs "$plan"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pl_check_ac_refs: spec present but no AC section -> every ref dangles (AC-10)" {
  spec="$TMP/spec.md"; plan="$TMP/plan.md"
  cat > "$spec" <<'EOF'
# Spec

## Problem

No acceptance criteria section at all.
EOF
  _write_plan_resolving "$plan"

  run pl_check_ac_refs "$plan" "$spec"
  [ "$status" -ne 0 ]
  # Both AC-1 and AC-2 are now dangling (spec declares zero ids).
  [ "$output" = "$(printf 'AC-1\nAC-2')" ]
}

@test "pl_check_ac_refs: plan with no ac_refs at all -> passes (nothing to dangle)" {
  spec="$TMP/spec.md"; plan="$TMP/plan.md"
  _write_spec_two "$spec"
  cat > "$plan" <<'EOF'
# Some Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "Legacy step", "check": "true" }
]
```
EOF

  run pl_check_ac_refs "$plan" "$spec"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
