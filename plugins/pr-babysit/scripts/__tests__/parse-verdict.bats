#!/usr/bin/env bats
# Real-data tests for parse-verdict.sh. The primary fixture (pr31-verdict.txt) is
# the captured real CI review bot comment from PR #31 — no mocks.

PV="${BATS_TEST_DIRNAME}/../parse-verdict.sh"
FIX="${BATS_TEST_DIRNAME}/fixtures"

@test "parse-verdict: PR#31 real comment is complete + approved" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  [ "$(jq -r .is_review_comment <<<"$out")" = "true" ]
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .complete <<<"$out")" = "true" ]
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "agent-merge-approved" ]
}

@test "parse-verdict: PR#31 yields exactly 6 findings, all low" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  [ "$(jq '.findings | length' <<<"$out")" -eq 6 ]
  [ "$(jq -r '[.findings[].severity] | unique | join(",")' <<<"$out")" = "low" ]
}

@test "parse-verdict: PR#31 parses path + line + key correctly" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  # first finding: session-start.sh line 17
  [ "$(jq -r '.findings[0].path' <<<"$out")" = "plugins/toolu/hooks/session-start.sh" ]
  [ "$(jq -r '.findings[0].line' <<<"$out")" = "17" ]
  # the bats finding has NO line number → null
  [ "$(jq -r '.findings[2].path' <<<"$out")" = "plugins/toolu/hooks/__tests__/session-start.bats" ]
  [ "$(jq -r '.findings[2].line' <<<"$out")" = "null" ]
  # keys are present and unique
  [ "$(jq -r '[.findings[].key] | length' <<<"$out")" -eq 6 ]
  [ "$(jq -r '[.findings[].key] | unique | length' <<<"$out")" -eq 6 ]
}

@test "parse-verdict: keys are stable across runs (deterministic)" {
  a=$(bash "$PV" < "$FIX/pr31-verdict.txt" | jq -c '[.findings[].key]')
  b=$(bash "$PV" < "$FIX/pr31-verdict.txt" | jq -c '[.findings[].key]')
  [ "$a" = "$b" ]
}

@test "parse-verdict: agent-merge-approved label wins over finding prose mentioning 'Changes requested'" {
  body=$'### Code Review — x\n\n- [x] done (`agent-merge-approved`)\n\n### Findings\n\n`a/b.sh:9`: low: prefer checking **Changes requested** before approved here.\n\n**Approved** (`agent-merge-approved`)'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "agent-merge-approved" ]
}

@test "parse-verdict: agent-merge-blocked label → changes" {
  body=$'### Code Review — x\n\n- [x] done\n\n### Findings\n\n`a/b.sh:9`: high: real problem.\n\n**Changes requested** (`agent-merge-blocked`)'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .verdict <<<"$out")" = "changes" ]
}

@test "parse-verdict: in-progress comment is not complete" {
  out=$(bash "$PV" < "$FIX/in-progress.txt")
  [ "$(jq -r .is_review_comment <<<"$out")" = "true" ]
  [ "$(jq -r .state <<<"$out")" = "in_progress" ]
  [ "$(jq -r .complete <<<"$out")" = "false" ]
}

@test "parse-verdict: comment with markers but no checklist is unknown (degrade)" {
  out=$(bash "$PV" < "$FIX/no-checkbox.txt")
  [ "$(jq -r .is_review_comment <<<"$out")" = "true" ]
  [ "$(jq -r .state <<<"$out")" = "unknown" ]
  [ "$(jq -r .complete <<<"$out")" = "false" ]
}

@test "parse-verdict: non-review / garbage input is not a review comment" {
  out=$(printf 'just a normal human comment, nothing here' | bash "$PV")
  [ "$(jq -r .is_review_comment <<<"$out")" = "false" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 0 ]
}

@test "parse-verdict: a bare actions/runs substring in prose is NOT a review comment" {
  out=$(printf 'fyi see actions/runs/999 and agent-merge-foo, thanks' | bash "$PV")
  [ "$(jq -r .is_review_comment <<<"$out")" = "false" ]
}

@test "parse-verdict: decorated '### Findings (N)' header still extracts findings" {
  body=$'### Code Review — x\n\n- [x] done\n\n### Findings (1)\n\n`a/b.sh:5`: low: a thing.\n\n### Other checks\n- ok'
  out=$(printf '%s' "$body" | bash "$PV")
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 1 ]
  [ "$(jq -r '.findings[0].path' <<<"$out")" = "a/b.sh" ]
}

@test "parse-verdict: a clean 'None' findings section is zero findings + approved" {
  body=$'### Code Review — x\n\n- [x] Reviewed\n\n### Findings\n\nNone — no blocking issues.\n\n**Approved** (`agent-merge-approved`)'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 0 ]
}

@test "parse-verdict: empty input is handled" {
  out=$(printf '' | bash "$PV")
  [ "$(jq -r .is_review_comment <<<"$out")" = "false" ]
}

# --- Current label shape (`merge-approved` / `request-changes`) ---------------
# The bot's verdict label was renamed from `agent-merge-*` at some point and the
# parser was never updated: both real comments below parsed to verdict "none",
# so babysit's success stop (which requires "approved") could never fire.

@test "parse-verdict: PR#122 real v6 comment → complete + approved via trailing chip" {
  out=$(bash "$PV" < "$FIX/pr122-verdict-approved.txt")
  [ "$(jq -r .is_review_comment <<<"$out")" = "true" ]
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "merge-approved" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 0 ]
}

@test "parse-verdict: PR#120 real comment → complete + changes via checklist label" {
  out=$(bash "$PV" < "$FIX/pr120-verdict-changes.txt")
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .verdict <<<"$out")" = "changes" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "request-changes" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 3 ]
}

@test "parse-verdict: '**Verdict:** ✅ Approved' prose is read when no label exists" {
  body=$'### Code Review — x\n\n- [x] done\n\n**Verdict:** ✅ Approved\n\n### Findings (0)\n\n_No findings._'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "" ]
}

@test "parse-verdict: '**Verdict:** ⚠️ Changes requested' prose is read when no label exists" {
  body=$'### Code Review — x\n\n- [x] done\n\n**Verdict:** ⚠️ Changes requested   🟡 2 medium\n\n### Findings\n\n`a/b.sh:9`: high: real problem.'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .verdict <<<"$out")" = "changes" ]
}

@test "parse-verdict: a finding quoting \`request-changes\` cannot shadow the checklist label" {
  body=$'### Code Review — x\n\n- [x] Set verdict label (`merge-approved`)\n\n### Findings\n\n`a/b.sh:9`: low: the label should be `request-changes` here.\n\n`merge-approved`'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  [ "$(jq -r .verdict_label <<<"$out")" = "merge-approved" ]
}

@test "parse-verdict: a bare trailing chip alone identifies a review comment" {
  body=$'some prose\n\n- [x] done\n\n`request-changes`'
  out=$(bash "$PV" <<<"$body")
  [ "$(jq -r .is_review_comment <<<"$out")" = "true" ]
  [ "$(jq -r .verdict <<<"$out")" = "changes" ]
}

# ── Top-N must-fix ─────────────────────────────────────────────────────────
#
# The bot populates `### Findings` and `### Top-N must-fix` independently, and
# they disagree in both directions. The fixture here is the real comment from
# PR #157 pass 1: `Findings (0)` while Top-N carried all three actionable
# items. A caller reading only findings[] sees nothing to do and stops.

@test "parse-verdict: Top-N items survive an empty findings section" {
  out=$(bash "$PV" < "$FIX/pr157-must-fix.txt")
  [ "$(jq '.findings | length' <<<"$out")" -eq 0 ]
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 3 ]
  [ "$(jq -r '.verdict' <<<"$out")" = "changes" ]
}

@test "parse-verdict: a must_fix line keeps its whole sentence" {
  out=$(bash "$PV" < "$FIX/pr157-must-fix.txt")
  [ "$(jq -r '.must_fix[0]' <<<"$out")" = "Fix pl_scope_sha to handle git diff failures explicitly" ]
  [ "$(jq -r '.must_fix[2]' <<<"$out")" = "Guard against path injection with leading dashes in git diff" ]
}

@test "parse-verdict: must_fix stops at the history block" {
  out=$(bash "$PV" < "$FIX/pr157-must-fix.txt")
  # The <details> history table follows Top-N; none of its rows may leak in.
  [ "$(jq -r '[.must_fix[] | select(test("Pass|d997dd5|---"))] | length' <<<"$out")" -eq 0 ]
}

@test "parse-verdict: list markers are stripped, sentences are not" {
  out=$(printf '%s\n' '### Code Review — `x`' '**Verdict:** ⚠️ Changes requested' \
    '### Top-N must-fix' 'Fix the bare line.' '- Fix the dash item.' \
    '* Fix the star item.' '1. Fix the numbered item.' '<details>' '`request-changes`' \
    | bash "$PV")
  [ "$(jq -r '.must_fix | join("|")' <<<"$out")" = "Fix the bare line.|Fix the dash item.|Fix the star item.|Fix the numbered item." ]
}

@test "parse-verdict: an approved verdict can still carry must_fix" {
  # Observed on PR #157's final pass: approved, zero findings, Top-N populated.
  out=$(printf '%s\n' '### Code Review — `x`' '**Verdict:** ✅ Approved' \
    '### Findings (0)' '### Top-N must-fix' 'Something the bot still wants.' \
    '<details>' '`merge-approved`' | bash "$PV")
  [ "$(jq -r '.verdict' <<<"$out")" = "approved" ]
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 1 ]
}

@test "parse-verdict: no Top-N section yields an empty list, not an error" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  [ "$(jq -r '.must_fix | type' <<<"$out")" = "array" ]
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 0 ]
}

# ── provider error: the action ran, the model did not ──────────────────────
#
# Captured from a real run on PR #159. The check reported SUCCESS and the
# comment carried `request-changes`, so a caller reading only the verdict sees
# an ordinary "changes with no findings" — indistinguishable from a review that
# ran and found nothing actionable. It was transient; the rerun produced a real
# review. That is exactly why it must be legible: a one-off failure that looks
# like a considered judgement is worse than one that announces itself.

@test "parse-verdict: a provider error is its own state, not a verdict" {
  out=$(bash "$PV" < "$FIX/provider-error.txt")
  [ "$(jq -r .state <<<"$out")" = "provider_error" ]
  [ "$(jq -r .complete <<<"$out")" = "false" ]
}

@test "parse-verdict: a provider error yields no findings and no must_fix" {
  out=$(bash "$PV" < "$FIX/provider-error.txt")
  [ "$(jq '.findings | length' <<<"$out")" -eq 0 ]
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 0 ]
}

@test "parse-verdict: a real review is never mistaken for a provider error" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .complete <<<"$out")" = "true" ]
}

@test "parse-verdict: a partial write cannot clobber collected must_fix items" {
  # The accumulator used `x=$(jq ...) || continue`, which assigns first and
  # branches second — a jq failure replaced the whole array with empty output
  # instead of skipping one line. Many lines must survive intact.
  out=$(printf '%s\n' '### Code Review — `x`' '**Verdict:** ⚠️ Changes requested' \
    '### Top-N must-fix' 'First item.' 'Second item.' 'Third item.' \
    '<details>' '`request-changes`' | bash "$PV")
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 3 ]
  [ "$(jq -r '.must_fix[0]' <<<"$out")" = "First item." ]
}

@test "parse-verdict: a finding that quotes the provider-error phrase is not a provider error" {
  # Self-referential hazard: a review OF this feature contains the phrase in
  # prose. Detection reads the verdict line, so quoting it changes nothing.
  out=$(printf '%s\n' '### Code Review — `x`' '**Verdict:** ⚠️ Changes requested' \
    '### Findings (1)' \
    '`a.sh:1`: medium: the action prints **Provider error:** when the model fails' \
    '<details>' '`request-changes`' | bash "$PV")
  [ "$(jq -r .state <<<"$out")" != "provider_error" ]
  [ "$(jq '.findings | length' <<<"$out")" -eq 1 ]
}

@test "parse-verdict: the provider-error verdict line is still detected" {
  out=$(bash "$PV" < "$FIX/provider-error.txt")
  [ "$(jq -r .state <<<"$out")" = "provider_error" ]
}

@test "parse-verdict: a decorated Top-N heading is matched" {
  out=$(printf '%s\n' '### Code Review — `x`' '**Verdict:** ⚠️ Changes requested' \
    '### Top-N must-fix (3)' 'One item.' '<details>' '`request-changes`' | bash "$PV")
  [ "$(jq '.must_fix | length' <<<"$out")" -eq 1 ]
}

@test "parse-verdict: real fixtures keep their state after the anchoring change" {
  out=$(bash "$PV" < "$FIX/pr31-verdict.txt")
  [ "$(jq -r .state <<<"$out")" = "complete" ]
  [ "$(jq -r .verdict <<<"$out")" = "approved" ]
  out2=$(bash "$PV" < "$FIX/pr157-must-fix.txt")
  [ "$(jq -r .state <<<"$out2")" = "complete" ]
  [ "$(jq -r .verdict <<<"$out2")" = "changes" ]
}
