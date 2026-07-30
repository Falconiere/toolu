#!/usr/bin/env bats
# Contract tests for the pr-babysit command prose + its real-data assumptions.
# The command is prose, so these assert the corrected instructions are present
# (grep invariants) and pin the empirical facts the design relies on against a
# captured real reviewThreads fixture from PR #115.

CMD="${BATS_TEST_DIRNAME}/../../commands/babysit.md"
FIXTURE="${BATS_TEST_DIRNAME}/fixtures/pr115-threads.json"

@test "command file exists" {
  [ -f "$CMD" ]
}

@test "defines a CI_REVIEWER login set" {
  grep -q 'CI_REVIEWER' "$CMD"
}

@test "names the GraphQL github-actions login (no [bot] suffix) explicitly" {
  grep -q 'github-actions' "$CMD"
}

@test "does NOT rely on a generic [bot] substring for the CI reviewer" {
  # The fragile heuristic that caused the bug must be gone.
  ! grep -qE 'login has .?\[bot\].? *$' "$CMD"
}

@test "does NOT instruct posting a per-round/summary conversation comment" {
  ! grep -qiE 'per-round summary|summary conversation comment' "$CMD"
}

@test "keeps the inline review-thread reply endpoint" {
  grep -qE 'comments/\{(root_comment_database_id|databaseId)\}/replies' "$CMD"
}

@test "has a round-level recurrence gate" {
  grep -qi 'recurrence gate' "$CMD"
}

@test "the recurrence gate does NOT suppress this round's replies" {
  # Strict clearance wins over the gate: reply+resolve first, then decide the stop.
  grep -qi 'never suppresses replies' "$CMD"
  ! grep -qi 'before posting any replies' "$CMD"
}

@test "states the strict-clearance invariant" {
  grep -qi 'strict-clearance invariant' "$CMD"
}

@test "resolves threads whose comment does not make sense" {
  grep -qi 'does not make sense' "$CMD"
  # The old behaviour parked ambiguous threads open for the reviewer.
  ! grep -qi 'NOT unclear ones' "$CMD"
  ! grep -qiE 'Unclear:.*clarif' "$CMD"
}

@test "resolves both dispositions, not just accepted ones" {
  grep -qi 'Resolve every thread you replied to' "$CMD"
}

@test "severity is not a filter for actioning findings" {
  grep -qi 'Severity is not a filter' "$CMD"
}

@test "scopes resolve to review threads (conversation comments have no thread)" {
  # GitHub exposes resolveReviewThread for review threads only — strict clearance
  # must not imply an issue comment can be resolved.
  grep -qi 'Resolve applies to review threads' "$CMD"
  grep -qi 'Conversation and review-level comments have no thread' "$CMD"
}

@test "has an end-of-round clearance check" {
  grep -qi 'Clearance check' "$CMD"
}

@test "instructs skipping outdated CI-reviewer threads" {
  grep -qiE 'outdated.*skip|skip silently' "$CMD"
}

@test "CI_REVIEWER covers both API forms for each app (no-suffix + [bot])" {
  grep -q 'github-actions, github-actions\[bot\], claude, claude\[bot\]' "$CMD"
}

@test "retains the verdict-approved success gate" {
  grep -qiE 'verdict.*approved' "$CMD"
}

@test "defines a resolution audit independent of the actionable filter" {
  grep -qi 'Resolution audit' "$CMD"
  grep -qi 'staleUnresolved' "$CMD"
}

@test "the clearance check runs the resolution audit, not the actionable filter" {
  grep -qi 'Re-run the .*Resolution audit.* from Step 1 — never the Step 1 actionable filter' "$CMD"
  # The stale bug this pins: reusing the last-comment filter as the resolved check.
  ! grep -qi 'Re-run the Step 1 filter\. Every actionable thread must now be resolved' "$CMD"
}

@test "the success stop is gated on the resolution audit, not the actionable filter" {
  grep -qi 're-run the .*Resolution audit.* (Step 1), never the actionable' "$CMD"
}

@test "resolve mutation failures are confirmed and retried, never assumed" {
  grep -qi "Confirm, don.t assume" "$CMD"
  grep -qi 'retry immediately' "$CMD"
}

@test "fixture is a non-empty array" {
  jq -e 'type=="array" and length>=1' "$FIXTURE" >/dev/null
}

@test "fixture pins the GraphQL github-actions login (no suffix)" {
  jq -e 'map(select(.login=="github-actions")) | length>=1' "$FIXTURE" >/dev/null
}

@test "fixture contains an isOutdated thread (skip-silently case)" {
  jq -e 'map(select(.isOutdated==true)) | length>=1' "$FIXTURE" >/dev/null
}
