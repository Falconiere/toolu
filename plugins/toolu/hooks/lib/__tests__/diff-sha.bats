#!/usr/bin/env bats
# Tests for hooks/lib/diff-sha.sh — the shared toolu_diff_sha helper.
# Real temp git repos, no mocked commands.
#
# Push-review, plan-ledger and docs-sync call-site behavior is covered by
# the native gate golden replay in pre-tool-modules-c.test.ts.

source_lib() {
  # shellcheck disable=SC1091
  . "${BATS_TEST_DIRNAME}/../diff-sha.sh"
}

# Raw formula every call site used before migrating to toolu_diff_sha.
raw_diff_sha() {
  git -C "$1" diff --no-color "${2}...HEAD" | git hash-object --stdin
}

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -q -b main
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
}

teardown() {
  rm -rf "$TMP"
}

# --- (a) byte-identical to the raw formula ----------------------------------

@test "toolu_diff_sha: byte-identical to the raw formula for a repo with changes" {
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "$(raw_diff_sha "$REPO" main)" ]
}

@test "toolu_diff_sha: byte-identical to the raw formula after commit --amend" {
  ( cd "$REPO" && git commit -q --amend --no-edit )
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "$(raw_diff_sha "$REPO" main)" ]
}

# --- (b) empty diff -----------------------------------------------------
#
# NOTE (deviation from the literal AC-11 prose, flagged rather than silently
# resolved): `git hash-object --stdin` on an empty diff stream succeeds and
# prints the well-known empty-blob sha (e69de29...) — a real, non-empty
# 40-char value, not "empty output". Spec component 1 is explicit that
# toolu_diff_sha does "NO sentinel mapping" and only fails on "git failure OR
# empty output"; treating the empty-blob sha as a failure would itself BE a
# sentinel mapping, and would break push-review.sh's own `== $EMPTY_BLOB_SHA`
# check (it needs to SEE that exact value to apply its own sentinel). So an
# empty diff is a SUCCESS here (exit 0, prints the empty-blob sha) — matching
# the pre-migration pl_diff_sha behavior exactly. AC-11's "returns non-zero on
# empty diff" holds at the CALL-SITE level instead (see the push-review parity
# test below, which still denies on an empty diff via its own sentinel) — its
# parenthetical ("call sites keep their local sentinel/allow behavior")
# supports this reading.

@test "toolu_diff_sha: an empty diff succeeds and prints the well-known empty-blob sha" {
  ( cd "$REPO" && git checkout -q main && git checkout -q -b feat/empty )
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391" ]
}

# --- (c) nonexistent base ref: non-zero -------------------------------------

@test "toolu_diff_sha: non-zero on a nonexistent base ref" {
  source_lib
  run toolu_diff_sha "$REPO" does-not-exist
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

# plan-ledger.sh (lib CLI, pl_diff_sha): a full run/status round trip stamps
# and re-recognizes fresh-green using the shared helper under the hood.
@test "parity (lib plan-ledger.sh CLI): run then status agree the ledger is fresh-green" {
  SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"
  doc="$REPO/plan.md"
  cat > "$doc" <<'EOF'
# Fixture Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First step", "check": "true" }
]
```
EOF
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  [[ "$output" == *"1/1 fresh-green"* ]]
}
