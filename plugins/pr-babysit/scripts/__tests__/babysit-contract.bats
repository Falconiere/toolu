#!/usr/bin/env bats
# Contract tests for the pr-babysit command prose + its real-data assumptions.
# The command is prose, so these assert the corrected instructions are present
# (grep invariants) and pin the empirical facts the design relies on against a
# captured real reviewThreads fixture from PR #115.

CMD="${BATS_TEST_DIRNAME}/../../workflows/babysit.md"
WRAPPER="${BATS_TEST_DIRNAME}/../../commands/babysit.md"
FIXTURE="${BATS_TEST_DIRNAME}/fixtures/pr115-threads.json"

@test "command file exists" {
  [ -f "$CMD" ]
  grep -Fq 'workflows/babysit.md' "$WRAPPER"
}

@test "skill accepts a verified execution handoff without changing its no-argument interface" {
  local skill="$BATS_TEST_DIRNAME/../../skills/babysit/SKILL.md"
  grep -qi 'verified execution handoff' "$skill"
  grep -qi 'sufficient authorization' "$skill"
  grep -qi 'no-argument invocation' "$skill"
  ! grep -qiE -- '--handoff|--from-execution' "$skill"
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

# --- shipped helper + trust boundary (issue #185) -----------------------------

HELPER_DOC="${BATS_TEST_DIRNAME}/../../skills/babysit/references/helper.md"
SKILL="${BATS_TEST_DIRNAME}/../../skills/babysit/SKILL.md"

@test "both host branches run the shipped tick helper, not inline gh/GraphQL fetches" {
  grep -Fq 'scripts/babysit-tick.sh' "$CMD"
  grep -Fq 'scripts/babysit-tick.sh' "$WRAPPER"
  grep -Fq 'scripts/babysit-tick.sh' "$SKILL"
  # The tick path carries no raw fetch the agent could copy-paste and drift on.
  ! grep -qE 'gh api graphql -f query=' "$CMD"
  ! grep -qE 'gh api repos/\{owner\}/\{repo\}/issues/\{number\}/comments *$' "$CMD"
  ! grep -qE 'gh pr checks' "$CMD"
}

@test "workflow, skill and command state the trust boundary and forbid home-grown controllers" {
  grep -qi 'Trust boundary' "$CMD"
  grep -qi 'Never write a polling script or controller of your own' "$CMD"
  grep -qi 'Never re-fetch what the result reports' "$CMD"
  grep -qi 'overridden only by naming the result field' "$CMD"
  for f in "$SKILL" "$WRAPPER"; do
    grep -qi 'never write a polling script or controller of your own' "$f"
    grep -qi 'never re-fetch' "$f"
    grep -Fq 'references/helper.md' "$f"
  done
  # No surface tells the agent to build the machinery itself: every line that
  # mentions writing a script/controller is a prohibition ("never ...").
  for f in "$CMD" "$SKILL" "$WRAPPER"; do
    ! grep -iE '(write|create|implement) (a |your own |the )?(python |bash )?(polling )?(script|controller)' "$f" | grep -viq 'never'
  done
}

@test "the write side goes through the helper: reply-thread, resolve-thread, record" {
  grep -Fq 'scripts/reply-thread.sh' "$CMD"
  grep -Fq 'scripts/resolve-thread.sh' "$CMD"
  grep -Fq 'record.sh round' "$CMD"
  grep -Fq 'record.sh flag-injection' "$CMD"
  grep -Fq 'record.sh status' "$CMD"
  grep -Fq 'duplicate_reply' "$CMD"
  grep -Fq 'resolve_unconfirmed' "$CMD"
  grep -Fq -- '--body-file' "$CMD"
}

@test "the workflow reads decisions from the result fields the reducer emits" {
  for field in 'decision' 'reasons\[\]' 'threads.actionable\[\]' 'threads.staleUnresolved\[\]' 'threads.skippedOutdated\[\]' 'threads.flaggedInjection\[\]' 'threads.unresolved' 'ci.status' 'ci.checks\[\]' 'sameRunAsLastTick' 'backoff.waitSeconds' 'backoff.intervalMinutes' 'recurrence.recurringKeys' 'mustFix\[\]'; do
    grep -qE -- "$field" "$CMD"
  done
  for code in pr_merged pr_closed merge_conflict fix_attempts_exhausted recurrence_after_rejection recurrence_streak provider_error_repeated ci_pending review_in_progress mergeable_unknown manual_verify unchanged; do
    grep -Fq "$code" "$CMD"
  done
}

@test "AC-16: Step 3 routes each fix by the model-routing rubric through both hosts' delegation interfaces" {
  step3=$(awk '/^## Step 3/{f=1} /^## Step 4/{f=0} f' "$CMD")
  grep -qi 'Model routing for fixes' <<<"$step3"
  grep -q 'model-routing' <<<"$step3"
  for tier in haiku sonnet opus mechanical implementation architecture; do grep -q "$tier" <<<"$step3"; done
  grep -q 'spawn_agent' <<<"$step3"
  grep -q '`Agent`' <<<"$step3"
  grep -q 'toolu:implementer' <<<"$step3"
  grep -qi 'Deciding and doing are different classes' <<<"$step3"
}

@test "references/helper.md documents every result and state field the workflow names" {
  [ -f "$HELPER_DOC" ]
  for f in slot changed decision 'reasons\[\]' ci.status 'ci.checks\[\]' verdict.state verdict.verdict verdict.findingsCount 'findingKeys\[\]' 'mustFix\[\]' verdict.degraded degradedReason sameRunAsLastTick threads.total 'threads.actionable\[\]' 'threads.staleUnresolved\[\]' 'threads.skippedOutdated\[\]' 'threads.flaggedInjection\[\]' 'conversation.actionable\[\]' 'reviews.actionable\[\]' recurrence backoff 'errors\[\]' snapshotPath statePath; do
    grep -qE -- "$f" "$HELPER_DOC"
  done
  for f in cronName lastUpdate totalTicks idleStreak currentInterval waitSeconds status worktree pr.key ciStatus reviewDecision mergeable unresolvedThreads headSha fixAttempts botVerdict botState botCommentId botCommentUpdatedAt botFindingKeys lastRoundFindingKeys lastRoundHadRejection recurrenceStreak unresolvedAfterClearance lastError actions.replied actions.resolved actions.flagged lastGoodSnapshot; do
    grep -Fq -- "$f" "$HELPER_DOC"
  done
  # One captured example per decision value, plus the structured-error shape.
  grep -Fq '"decision":"success"' "$HELPER_DOC"
  grep -Fq '"decision":"keep_going"' "$HELPER_DOC"
  grep -Fq '"decision":"escalate"' "$HELPER_DOC"
  grep -Fq '"code":"locked"' "$HELPER_DOC"
  for code in usage gh_unavailable jq_required api_error invalid_json head_moved state_malformed slot_mismatch locked duplicate_reply resolve_unconfirmed; do
    grep -Fq "$code" "$HELPER_DOC"
  done
}

@test "the closed reason set in helper.md matches the reducer's emitted codes" {
  for code in ci_pending ci_failed ci_pass threads_unresolved threads_stale_unresolved threads_clear review_absent review_in_progress review_changes review_approved review_unknown_format provider_error provider_error_repeated manual_verify pr_closed pr_merged merge_conflict mergeable_unknown fix_attempts_exhausted recurrence_after_rejection recurrence_streak unchanged; do
    grep -Fq "$code" "$HELPER_DOC"
    grep -Fq "\"$code\"" "${BATS_TEST_DIRNAME}/../reduce-state.sh"
  done
}

@test "detached worktree push is documented end to end: --branch for the writer, HEAD:<branch> for the push" {
  grep -Fq -- '--branch "$BRANCH"' "$CMD"
  grep -Fq 'push origin "HEAD:$BRANCH"' "$CMD"
  grep -Fq -- '--branch <name>' "${BATS_TEST_DIRNAME}/../../../toolu-review/skills/review/SKILL.md"
}

