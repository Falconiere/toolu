#!/usr/bin/env bats
# Tests for hooks/lib/push-waiver.sh.
#
# Every SHA here is a real `git diff | git hash-object` value from a real
# repository built in setup — the waiver's whole contract is "this exact diff",
# so a hand-written SHA would test nothing about the code that produces one.

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  cd "$REPO"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  echo "one" > a.txt
  git add a.txt
  git commit -qm "feat: one"
  git checkout -qb feat/x
  echo "two" > b.txt
  git add b.txt
  git commit -qm "feat: two"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../diff-sha.sh
  . "$REPO_ROOT/hooks/lib/diff-sha.sh"
  # shellcheck source=../push-waiver.sh
  . "$REPO_ROOT/hooks/lib/push-waiver.sh"

  export STATE_DIR="$TMP/state/push-review"
  SHA=$(toolu_diff_sha "$REPO" main)
  SLUG=feat_x
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Add a commit so the branch diff — and therefore its SHA — really changes.
add_commit() {
  echo "three" > c.txt
  git add c.txt
  git commit -qm "feat: three"
}

@test "the fixture produces a real, non-empty diff sha" {
  [ -n "$SHA" ]
  [ "$SHA" != "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391" ]
}

@test "no waiver exists before one is written" {
  run push_waiver_matches "$REPO" "$SLUG" "$SHA"
  [ "$status" -ne 0 ]
}

@test "pend then promote produces a waiver matching the diff sha" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  [ -f "$STATE_DIR/$SLUG.pending-waiver.json" ]
  push_waiver_promote "$REPO" "$SLUG" "$SHA"
  run push_waiver_matches "$REPO" "$SLUG" "$SHA"
  [ "$status" -eq 0 ]
}

@test "promotion removes the pending marker" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  push_waiver_promote "$REPO" "$SLUG" "$SHA"
  [ ! -f "$STATE_DIR/$SLUG.pending-waiver.json" ]
}

@test "the waiver records the reason code it was asked about" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main stale-diff
  push_waiver_promote "$REPO" "$SLUG" "$SHA"
  [ "$(jq -r '.reason_code' "$STATE_DIR/$SLUG.waiver.json")" = "stale-diff" ]
  [ "$(jq -r '.base_branch' "$STATE_DIR/$SLUG.waiver.json")" = "main" ]
  [ "$(jq -r '.waived_at' "$STATE_DIR/$SLUG.waiver.json")" != "null" ]
  [ "$(jq -r '.asked_at // "gone"' "$STATE_DIR/$SLUG.waiver.json")" = "gone" ]
}

@test "promotion refuses a pending marker from a different diff" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  add_commit
  new_sha=$(toolu_diff_sha "$REPO" main)
  [ "$new_sha" != "$SHA" ]
  run push_waiver_promote "$REPO" "$SLUG" "$new_sha"
  [ "$status" -ne 0 ]
  [ ! -f "$STATE_DIR/$SLUG.waiver.json" ]
  [ -f "$STATE_DIR/$SLUG.pending-waiver.json" ]
}

@test "a waiver stops matching once the diff changes" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  push_waiver_promote "$REPO" "$SLUG" "$SHA"
  add_commit
  new_sha=$(toolu_diff_sha "$REPO" main)
  run push_waiver_matches "$REPO" "$SLUG" "$new_sha"
  [ "$status" -ne 0 ]
}

@test "promotion with no pending marker writes nothing" {
  run push_waiver_promote "$REPO" "$SLUG" "$SHA"
  [ "$status" -ne 0 ]
  [ ! -f "$STATE_DIR/$SLUG.waiver.json" ]
}

@test "a corrupt waiver reads as no waiver" {
  mkdir -p "$STATE_DIR"
  printf 'not json' > "$STATE_DIR/$SLUG.waiver.json"
  run push_waiver_matches "$REPO" "$SLUG" "$SHA"
  [ "$status" -ne 0 ]
}

@test "a waiver from an unknown schema version reads as no waiver" {
  mkdir -p "$STATE_DIR"
  jq -cn --arg sha "$SHA" '{version: 99, diff_sha: $sha}' > "$STATE_DIR/$SLUG.waiver.json"
  run push_waiver_matches "$REPO" "$SLUG" "$SHA"
  [ "$status" -ne 0 ]
}

@test "an empty sha never matches a waiver" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  push_waiver_promote "$REPO" "$SLUG" "$SHA"
  run push_waiver_matches "$REPO" "$SLUG" ""
  [ "$status" -ne 0 ]
}

@test "pending markers are per branch" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  run push_waiver_promote "$REPO" "other_branch" "$SHA"
  [ "$status" -ne 0 ]
}

@test "asking again overwrites the earlier pending marker" {
  push_waiver_pend "$REPO" "$SLUG" "$SHA" main no-state
  add_commit
  new_sha=$(toolu_diff_sha "$REPO" main)
  push_waiver_pend "$REPO" "$SLUG" "$new_sha" main findings
  [ "$(jq -r '.diff_sha' "$STATE_DIR/$SLUG.pending-waiver.json")" = "$new_sha" ]
  push_waiver_promote "$REPO" "$SLUG" "$new_sha"
  run push_waiver_matches "$REPO" "$SLUG" "$new_sha"
  [ "$status" -eq 0 ]
}

@test "waiver paths land under the push-review state dir" {
  [ "$(push_waiver_path "$REPO" "$SLUG")" = "$STATE_DIR/$SLUG.waiver.json" ]
  [ "$(push_waiver_pending_path "$REPO" "$SLUG")" = "$STATE_DIR/$SLUG.pending-waiver.json" ]
}
