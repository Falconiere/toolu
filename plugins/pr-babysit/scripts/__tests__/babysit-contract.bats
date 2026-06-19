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

@test "has a round-level recurrence gate before replies" {
  grep -qiE 'recurrence gate|before posting any repl' "$CMD"
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

@test "fixture is a non-empty array" {
  jq -e 'type=="array" and length>=1' "$FIXTURE" >/dev/null
}

@test "fixture pins the GraphQL github-actions login (no suffix)" {
  jq -e 'map(select(.login=="github-actions")) | length>=1' "$FIXTURE" >/dev/null
}

@test "fixture contains an isOutdated thread (skip-silently case)" {
  jq -e 'map(select(.isOutdated==true)) | length>=1' "$FIXTURE" >/dev/null
}
