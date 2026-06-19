#!/usr/bin/env bats
# Tests for hooks/lib/plan-ledger-parse.sh — steps-block parse + atomic ledger I/O.

bats_require_minimum_version 1.5.0

setup() {
  TMP=$(mktemp -d)
  # shellcheck source=../plan-ledger-parse.sh
  . "${BATS_TEST_DIRNAME}/../plan-ledger-parse.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Write a plan doc carrying a valid 2-step machine-readable block, plus prose
# before and after to prove the parser is anchored on the heading + fences.
_write_valid_doc() {
  cat > "$1" <<'EOF'
# Some Plan

## Context

Prose that mentions a ```json block elsewhere should be ignored:

```json
{ "decoy": true }
```

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First step", "check": "true" },
  { "id": "s2", "title": "Second step", "check": "bats foo.bats" }
]
```

## Verification

More prose after the block.
EOF
}

@test "pl_parse_steps: valid 2-step block parses to a length-2 array with matching ids" {
  doc="$TMP/plan.md"
  _write_valid_doc "$doc"

  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]

  echo "$output" | jq -e 'length == 2'
  [ "$(echo "$output" | jq -r '.[0].id')" = "s1" ]
  [ "$(echo "$output" | jq -r '.[1].id')" = "s2" ]
  [ "$(echo "$output" | jq -r '.[1].check')" = "bats foo.bats" ]
}

@test "pl_parse_steps: ignores a decoy json block that precedes the marker heading" {
  doc="$TMP/plan.md"
  _write_valid_doc "$doc"

  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]
  # The decoy object must not leak in; we get the steps array, not {decoy:true}.
  [ "$(echo "$output" | jq -r 'type')" = "array" ]
}

@test "pl_parse_steps: missing steps heading -> non-zero, nothing on stdout" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
# Some Plan

## Context

No machine-readable steps here.
EOF

  # --separate-stderr so $output is stdout only; the lib's diagnostic goes to
  # $stderr, and the contract ("nothing on stdout") is asserted precisely.
  run --separate-stderr pl_parse_steps "$doc"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

@test "pl_parse_steps: empty array block -> non-zero" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
## Steps (machine-readable)

```json
[]
```
EOF

  run pl_parse_steps "$doc"
  [ "$status" -ne 0 ]
}

@test "pl_parse_steps: malformed json -> non-zero" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
## Steps (machine-readable)

```json
[{"id":
```
EOF

  run pl_parse_steps "$doc"
  [ "$status" -ne 0 ]
}

@test "pl_parse_steps: step missing 'check' field -> non-zero" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First step" }
]
```
EOF

  run pl_parse_steps "$doc"
  [ "$status" -ne 0 ]
}

@test "pl_parse_steps: step with empty-string id -> non-zero" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
## Steps (machine-readable)

```json
[
  { "id": "", "title": "First step", "check": "true" }
]
```
EOF

  run pl_parse_steps "$doc"
  [ "$status" -ne 0 ]
}

@test "pl_parse_steps: absent file -> non-zero, nothing on stdout" {
  run --separate-stderr pl_parse_steps "$TMP/does-not-exist.md"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

@test "pl_parse_steps: normalizes ac_refs/depends_on/input; legacy step gets []/[]/null (AC-1)" {
  doc="$TMP/plan.md"
  cat > "$doc" <<'EOF'
# Enriched Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "Enriched step", "check": "true",
    "ac_refs": ["AC-1", "AC-2"], "depends_on": ["s0"], "input": "fixture docs" },
  { "id": "s2", "title": "Legacy step", "check": "false" }
]
```
EOF

  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]

  # Enriched step carries its authored values through unchanged.
  [ "$(echo "$output" | jq -c '.[0].ac_refs')" = '["AC-1","AC-2"]' ]
  [ "$(echo "$output" | jq -c '.[0].depends_on')" = '["s0"]' ]
  [ "$(echo "$output" | jq -r '.[0].input')" = "fixture docs" ]

  # Legacy {id,title,check} step gets the defaults []/[]/null.
  [ "$(echo "$output" | jq -c '.[1].ac_refs')" = '[]' ]
  [ "$(echo "$output" | jq -c '.[1].depends_on')" = '[]' ]
  [ "$(echo "$output" | jq -r '.[1].input')" = "null" ]
  # And its core fields are untouched.
  [ "$(echo "$output" | jq -r '.[1].id')" = "s2" ]
  [ "$(echo "$output" | jq -r '.[1].check')" = "false" ]
}

@test "pl_read_ledger: round-trips json written by pl_write_ledger (jq -S equal)" {
  ledger="$TMP/nested/dir/feat-x.json"
  sample='{"version":1,"branch":"feat/x","steps":[{"id":"s1","status":"green"}]}'

  run pl_write_ledger "$ledger" "$sample"
  [ "$status" -eq 0 ]
  [ -f "$ledger" ]

  run pl_read_ledger "$ledger"
  [ "$status" -eq 0 ]

  want=$(echo "$sample" | jq -S .)
  got=$(echo "$output" | jq -S .)
  [ "$want" = "$got" ]
}

@test "pl_write_ledger: creates missing parent directories" {
  ledger="$TMP/a/b/c/ledger.json"
  pl_write_ledger "$ledger" '{"version":1}'
  [ -f "$ledger" ]
}

@test "pl_write_ledger: leaves no .tmp.* file behind after a successful write" {
  ledger="$TMP/ledger.json"
  pl_write_ledger "$ledger" '{"version":1}'
  # No partial temp file should remain in the directory.
  run bash -c "ls -1 \"$TMP\"/*.tmp.* 2>/dev/null"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

@test "pl_write_ledger: invalid json -> non-zero, no file written" {
  ledger="$TMP/ledger.json"
  run pl_write_ledger "$ledger" '{"version":'
  [ "$status" -ne 0 ]
  [ ! -f "$ledger" ]
}

@test "pl_write_ledger: invalid json leaves no .tmp.* file behind" {
  ledger="$TMP/ledger.json"
  run pl_write_ledger "$ledger" 'not json at all'
  [ "$status" -ne 0 ]
  run bash -c "ls -1 \"$TMP\"/*.tmp.* 2>/dev/null"
  [ "$status" -ne 0 ]
}

@test "pl_read_ledger: absent file -> non-zero" {
  run pl_read_ledger "$TMP/missing.json"
  [ "$status" -ne 0 ]
}

@test "pl_read_ledger: empty file -> non-zero" {
  ledger="$TMP/empty.json"
  : > "$ledger"
  run pl_read_ledger "$ledger"
  [ "$status" -ne 0 ]
}

# --- additive schema (a1): started_at / activity / running (version stays 1) ---
# These exercise pl_build_step_entry + pl_recompute from plan-ledger.sh, which
# sources this parse lib. Sourcing it here pulls in the new helpers alongside the
# parse/IO functions already loaded by setup().
_source_ledger() {
  # shellcheck source=../plan-ledger.sh
  . "${BATS_TEST_DIRNAME}/../plan-ledger.sh"
}

@test "pl_build_step_entry: omitting started_at/activity defaults both to null" {
  _source_ledger
  steps='[{"id":"s1","title":"First","check":"true"}]'
  run pl_build_step_entry "$steps" s1 green 0 deadbeef '"tail"'
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.started_at')" = "null" ]
  [ "$(echo "$output" | jq -r '.activity')" = "null" ]
  # Existing fields still present and correct.
  [ "$(echo "$output" | jq -r '.status')" = "green" ]
  [ "$(echo "$output" | jq -r '.diff_sha')" = "deadbeef" ]
}

@test "pl_build_step_entry: started_at/activity passed through when given" {
  _source_ledger
  steps='[{"id":"s1","title":"First","check":"true"}]'
  run pl_build_step_entry "$steps" s1 running null "" 'null' "2026-06-17T00:00:00Z" "editing foo.ts"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.status')" = "running" ]
  [ "$(echo "$output" | jq -r '.started_at')" = "2026-06-17T00:00:00Z" ]
  [ "$(echo "$output" | jq -r '.activity')" = "editing foo.ts" ]
}

@test "pl_recompute: summary gains running count; running step is not fresh_green" {
  _source_ledger
  ledger='{"version":1,"steps":[
    {"id":"s1","status":"green","diff_sha":"abc"},
    {"id":"s2","status":"running","started_at":"2026-06-17T00:00:00Z","diff_sha":null},
    {"id":"s3","status":"pending","diff_sha":null}
  ]}'
  run pl_recompute "$ledger" "abc"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.summary.running')" = "1" ]
  [ "$(echo "$output" | jq -r '.summary.green')" = "1" ]
  [ "$(echo "$output" | jq -r '.summary.fresh_green')" = "1" ]
  # next is the first non-fresh step: the running s2.
  [ "$(echo "$output" | jq -r '.next')" = "s2" ]
  # version is untouched (stays 1).
  [ "$(echo "$output" | jq -r '.version')" = "1" ]
}
