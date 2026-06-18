#!/usr/bin/env bats
# Tests for `plan-ledger.sh preflight` (plan-ledger-preflight.sh): the AC#1
# precondition matrix. Real git sandbox + real plan/spec fixture docs, no mocks.

bats_require_minimum_version 1.5.0

SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO/docs/toolu/specs" "$REPO/docs/toolu/plans"
  (
    cd "$REPO"
    git init -b main -q
    git config user.email "t@example.com"
    git config user.name "Tester"
    echo base > base.txt
    git add base.txt
    git commit -qm base
    git checkout -q -b feat/x
  )
  export PUSH_REVIEW_BASE=main
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# $1=path $2=Status $3=Spec field literal (omit the arg to leave the Spec line off)
_write_plan() {
  local path="$1" status="$2"
  if [ "$#" -ge 3 ]; then
    printf '# Fixture Plan — Plan\n\n**Date:** 2026-06-17   **Status:** %s   **Spec:** %s   **Topic:** t\n' \
      "$status" "$3" > "$path"
  else
    printf '# Fixture Plan — Plan\n\n**Date:** 2026-06-17   **Status:** %s   **Topic:** t\n' \
      "$status" > "$path"
  fi
}

_write_spec() {
  printf '# Fixture Spec — Design\n\n**Date:** 2026-06-17   **Status:** %s   **Topic:** t\n' \
    "$2" > "$1"
}

preflight() {
  run bash -c "cd '$REPO' && bash '$SCRIPT' preflight '$1'"
}

# AC#1: plan Draft -> exit 1 naming the plan.
@test "preflight: plan Status Draft denies (exit 1) and names not-approved" {
  _write_plan "$REPO/docs/toolu/plans/p.md" Draft none
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 1 ]
  [[ "$output" == *"not approved"* ]]
  [[ "$output" == *"Draft"* ]]
}

# AC#1: plan Approved + spec Needs changes -> exit 1 naming the spec.
@test "preflight: plan Approved but spec Needs changes denies (exit 1) naming the spec" {
  _write_spec "$REPO/docs/toolu/specs/s.md" "Needs changes"
  _write_plan "$REPO/docs/toolu/plans/p.md" Approved docs/toolu/specs/s.md
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 1 ]
  [[ "$output" == *"docs/toolu/specs/s.md"* ]]
  [[ "$output" == *"not approved"* ]]
}

# AC#1: plan Approved + spec Approved -> exit 0 silently.
@test "preflight: plan Approved + spec Approved passes (exit 0, silent)" {
  _write_spec "$REPO/docs/toolu/specs/s.md" Approved
  _write_plan "$REPO/docs/toolu/plans/p.md" Approved docs/toolu/specs/s.md
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# AC#1: case-insensitive approval ("approved" lowercase).
@test "preflight: approval is case-insensitive" {
  _write_plan "$REPO/docs/toolu/plans/p.md" approved none
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# AC#1: plan Approved + **Spec:** none -> spec-less, exit 0.
@test "preflight: plan Approved with Spec none is spec-less and passes" {
  _write_plan "$REPO/docs/toolu/plans/p.md" Approved none
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# AC#1: plan Approved + NO **Spec:** line -> spec-less, exit 0.
@test "preflight: plan Approved with no Spec line is spec-less and passes" {
  _write_plan "$REPO/docs/toolu/plans/p.md" Approved
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# AC#1: declared-but-unreadable spec -> exit 1 (NOT exit 2).
@test "preflight: plan Approved but declared spec missing denies with exit 1" {
  _write_plan "$REPO/docs/toolu/plans/p.md" Approved docs/toolu/specs/missing.md
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 1 ]
  [[ "$output" == *"docs/toolu/specs/missing.md"* ]]
}

# Header-less / malformed plan: empty Status -> exit 1 (not 2).
@test "preflight: header-less plan (no Status) denies with exit 1" {
  printf '# Bare Plan\n\nNo header line at all.\n' > "$REPO/docs/toolu/plans/p.md"
  preflight docs/toolu/plans/p.md
  [ "$status" -eq 1 ]
  [[ "$output" == *"no **Status:** header"* ]]
}

# Plan doc missing entirely -> exit 2.
@test "preflight: missing plan doc -> exit 2" {
  preflight docs/toolu/plans/does-not-exist.md
  [ "$status" -eq 2 ]
}
