#!/usr/bin/env bats
# Tests for the optional per-step `model` tier: parse-time validation and
# backfill (plan-ledger-parse.sh), carry-through into the ledger, and the
# `model=<alias>` suffix on the status/run summary line (plan-ledger.sh).
# Real git sandbox, real checks. No mocks.

bats_require_minimum_version 1.5.0

SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -b main -q
    git config user.email "t@example.com"
    git config user.name "Tester"
    echo base > base.txt
    git add base.txt
    git commit -qm base
    git checkout -q -b feat/x
    echo feature > feature.txt
    git add feature.txt
    git commit -qm feature
  )
  export PUSH_REVIEW_BASE=main
  LEDGER="$REPO/.claude/tmp/plan-ledger/feat_x.json"
  # shellcheck source=../plan-ledger-parse.sh
  . "${BATS_TEST_DIRNAME}/../plan-ledger-parse.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# _write_doc PATH STEPS_JSON_BODY
_write_doc() {
  cat > "$1" <<EOF
# Fixture Plan

**Date:** 2026-07-29   **Status:** Approved   **Spec:** none   **Topic:** tiers

## Steps (machine-readable)

\`\`\`json
$2
\`\`\`
EOF
}

@test "parity: the parser's alias list matches TOOLU_MODEL_ALIASES in config.sh" {
  # plan-ledger-parse.sh is jq-only and must not source the config loader, so
  # the alias list is duplicated. This test is what keeps the two honest: a new
  # alias added to one side without the other fails here.
  local lib_dir="${BATS_TEST_DIRNAME}/.."
  # shellcheck source=../config.sh
  . "$lib_dir/config.sh"
  local from_parser
  from_parser=$(grep -o '\["haiku"[^]]*\]' "$lib_dir/plan-ledger-parse.sh" \
    | tr -d '"[]' | tr ',' ' ')
  [ -n "$from_parser" ]
  [ "$from_parser" = "$TOOLU_MODEL_ALIASES" ]
}

@test "parse: a legacy step with no model backfills to null" {
  doc="$TMP/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Legacy", "check": "true" }]'
  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.[0] | has("model")'
  [ "$(echo "$output" | jq -r '.[0].model')" = "null" ]
}

@test "parse: every routable alias is accepted and preserved verbatim" {
  doc="$TMP/plan.md"
  _write_doc "$doc" '[
    { "id": "s1", "title": "Cheap",  "check": "true", "model": "haiku" },
    { "id": "s2", "title": "Mid",    "check": "true", "model": "sonnet" },
    { "id": "s3", "title": "Top",    "check": "true", "model": "opus" },
    { "id": "s4", "title": "Fable",  "check": "true", "model": "fable" },
    { "id": "s5", "title": "Lead",   "check": "true", "model": "inherit" }
  ]'
  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '[.[].model] | join(",")')" = "haiku,sonnet,opus,fable,inherit" ]
}

@test "parse: an unroutable model is rejected with the offending step named" {
  doc="$TMP/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Bad", "check": "true", "model": "gpt-9" }]'
  run pl_parse_steps "$doc"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q 's1=gpt-9'
  echo "$output" | grep -q 'allowed: haiku sonnet opus fable inherit'
}

@test "parse: a non-string model is rejected, not coerced" {
  doc="$TMP/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Bad", "check": "true", "model": 5 }]'
  run pl_parse_steps "$doc"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q 'invalid step model'
}

@test "parse: an explicit null model is valid (means unrouted)" {
  doc="$TMP/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Unrouted", "check": "true", "model": null }]'
  run pl_parse_steps "$doc"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.[0].model')" = "null" ]
}

@test "run: the declared tier lands in the ledger entry" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Tiered", "check": "true", "model": "haiku" }]'
  cd "$REPO"
  run bash "$SCRIPT" run "$doc"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.steps[0].model' "$LEDGER")" = "haiku" ]
  [ "$(jq -r '.steps[0].status' "$LEDGER")" = "green" ]
}

@test "run: the summary line advertises the next step's tier" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[
    { "id": "s1", "title": "Green",   "check": "true",  "model": "haiku" },
    { "id": "s2", "title": "Pending", "check": "false", "model": "opus" }
  ]'
  cd "$REPO"
  run bash "$SCRIPT" run "$doc"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q 'next=s2 model=opus'
}

@test "status: reads the tier back without re-running checks" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[
    { "id": "s1", "title": "Green", "check": "true",  "model": "haiku" },
    { "id": "s2", "title": "Red",   "check": "false", "model": "sonnet" }
  ]'
  cd "$REPO"
  bash "$SCRIPT" run "$doc" || true
  run bash "$SCRIPT" status
  echo "$output" | grep -q 'next=s2 model=sonnet'
}

@test "summary: an unrouted next step keeps the legacy line shape" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Legacy", "check": "false" }]'
  cd "$REPO"
  run bash "$SCRIPT" run "$doc"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q 'fresh-green, next=s1$'
  ! echo "$output" | grep -q 'model='
}

@test "summary: no suffix when every step is fresh-green" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[{ "id": "s1", "title": "Done", "check": "true", "model": "opus" }]'
  cd "$REPO"
  run bash "$SCRIPT" run "$doc"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'next=none$'
}

@test "run: an edited tier is re-derived on the next run, not frozen in the ledger" {
  doc="$REPO/plan.md"
  _write_doc "$doc" '[
    { "id": "s1", "title": "One", "check": "true", "model": "haiku" },
    { "id": "s2", "title": "Two", "check": "true", "model": "haiku" }
  ]'
  cd "$REPO"
  bash "$SCRIPT" run "$doc"
  [ "$(jq -r '.steps[1].model' "$LEDGER")" = "haiku" ]
  # Re-tier s2 in the plan doc, then run only s1: s2's carried-forward entry
  # must pick up the authored change like the other authored fields do.
  _write_doc "$doc" '[
    { "id": "s1", "title": "One", "check": "true", "model": "haiku" },
    { "id": "s2", "title": "Two", "check": "true", "model": "opus" }
  ]'
  bash "$SCRIPT" run "$doc" --step s1
  [ "$(jq -r '.steps[1].model' "$LEDGER")" = "opus" ]
}
