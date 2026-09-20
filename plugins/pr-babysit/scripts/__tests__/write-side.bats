#!/usr/bin/env bats
# Real-data tests for the write side: record.sh, reply-thread.sh,
# resolve-thread.sh. Offline block: real state files produced by
# babysit-tick.sh from the captured #165 snapshot; refusal paths that must
# never make a request. Live block (PR_BABYSIT_LIVE_WRITE=1, opt-in, never in
# CI): creates a throwaway branch + PR + review thread on Falconiere/toolu with
# gh, exercises reply/resolve against it, and closes everything in teardown.

SCRIPTS="${BATS_TEST_DIRNAME}/.."
SNAP="${BATS_TEST_DIRNAME}/fixtures/snapshots"
NOW=2026-09-19T12:00:00Z

setup() {
  TMP=$(mktemp -d)
  STATE="$TMP/pr-babysit-falconiere-toolu-165.json"
  bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-165.json" --now "$NOW" >/dev/null
  printf 'Fixed in abc1234 — guarded the empty case.\n' >"$TMP/body.txt"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# --- record.sh ----------------------------------------------------------------

@test "record round: rotates finding keys, sets rejection flag, bumps fixAttempts only with --fix-pushed, caps at 5" {
  jq '.pr.botFindingKeys = ["a.sh:1:deadbeef","b.sh:2:cafebabe"]' "$STATE" >"$TMP/s.json" && mv "$TMP/s.json" "$STATE"
  out=$(bash "$SCRIPTS/record.sh" round --state-file "$STATE" --had-rejection true --fix-pushed)
  [ "$(jq -r .ok <<<"$out")" = true ]
  [ "$(jq -c .pr.lastRoundFindingKeys "$STATE")" = '["a.sh:1:deadbeef","b.sh:2:cafebabe"]' ]
  [ "$(jq -r .pr.lastRoundHadRejection "$STATE")" = true ]
  [ "$(jq -r .pr.fixAttempts "$STATE")" = 1 ]
  [ "$(jq -r .lastRound.fixPushed "$STATE")" = true ]
  bash "$SCRIPTS/record.sh" round --state-file "$STATE" --had-rejection false >/dev/null
  [ "$(jq -r .pr.lastRoundHadRejection "$STATE")" = false ]
  [ "$(jq -r .pr.fixAttempts "$STATE")" = 1 ]
  for _ in 1 2 3 4 5 6; do bash "$SCRIPTS/record.sh" round --state-file "$STATE" --had-rejection false --fix-pushed >/dev/null; done
  [ "$(jq -r .pr.fixAttempts "$STATE")" = 5 ]
  jq -e '.version == 2' "$STATE" >/dev/null
}

@test "record round feeds the reducer: recurrence and fix-attempt escalation come from recorded rounds, not polls" {
  jq '.pr.fixAttempts = 4' "$STATE" >"$TMP/s.json" && mv "$TMP/s.json" "$STATE"
  bash "$SCRIPTS/record.sh" round --state-file "$STATE" --had-rejection false --fix-pushed >/dev/null
  out=$(bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-165.json" --now 2026-09-19T12:03:00Z)
  [[ "$(jq -r '[.reasons[].code] | join(",")' <<<"$out")" == *fix_attempts_exhausted* ]]
  [ "$(jq -r .recurrence.fixAttempts <<<"$out")" = 5 ]
}

@test "record flag-injection: the reducer exempts the thread on the next tick" {
  id=$(jq -r '.threads[0].id' "$SNAP/toolu-165.json")
  bash "$SCRIPTS/record.sh" flag-injection --state-file "$STATE" --thread "$id" >/dev/null
  [ "$(jq -r --arg id "$id" '.actions.flagged[$id].reason' "$STATE")" = injection ]
  out=$(bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-165.json" --now 2026-09-19T12:03:00Z)
  [ "$(jq -c .threads.flaggedInjection <<<"$out")" = "[\"$id\"]" ]
}

@test "record status: terminal transition is the agent's, with a timestamp" {
  for s in complete escalated cancelled; do
    out=$(bash "$SCRIPTS/record.sh" status --state-file "$STATE" --status "$s")
    [ "$(jq -r .status <<<"$out")" = "$s" ]
    [ "$(jq -r .status "$STATE")" = "$s" ]
    [[ "$(jq -r .statusChangedAt "$STATE")" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T ]]
  done
  run bash "$SCRIPTS/record.sh" status --state-file "$STATE" --status done
  [ "$status" -eq 2 ]
}

@test "record: usage, missing state, malformed state, and a held lock are refused without writing" {
  cp "$STATE" "$TMP/before"
  run bash "$SCRIPTS/record.sh" round --state-file "$STATE"
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/record.sh" nope --state-file "$STATE"
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/record.sh" status --state-file "$TMP/missing.json" --status complete
  [ "$status" -eq 3 ]; [ "$(jq -r '.errors[0].code' <<<"$output")" = state_malformed ]
  printf 'garbage' >"$TMP/bad.json"
  run bash "$SCRIPTS/record.sh" status --state-file "$TMP/bad.json" --status complete
  [ "$status" -eq 3 ]; [ "$(cat "$TMP/bad.json")" = garbage ]
  bash -c ". '$SCRIPTS/lib/common.sh'; . '$SCRIPTS/lib/lock.sh'; pb_init; pb_lock_acquire '$STATE'; sleep 2" &
  holder=$!
  for _ in 1 2 3 4 5 6 7 8 9 10; do [ -f "$STATE.lock/since" ] && break; sleep 0.1; done
  run bash "$SCRIPTS/record.sh" status --state-file "$STATE" --status complete
  [ "$status" -eq 75 ]
  cmp "$TMP/before" "$STATE"
  kill "$holder" 2>/dev/null || true; wait "$holder" 2>/dev/null || true
}

# --- reply-thread.sh / resolve-thread.sh: refusals that make no request ------

@test "reply: a recorded key is refused with exit 4 before any request (works with no network)" {
  jq '.actions.replied["thread:PRRT_a@42"] = {commentId: 1, url: "u", at: "x"}' "$STATE" >"$TMP/s.json" && mv "$TMP/s.json" "$STATE"
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind thread --thread PRRT_a --root-comment 41 --in-reply-to 42 --body-file "$TMP/body.txt"
  [ "$status" -eq 4 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = duplicate_reply ]
  [ "$(jq -r '.errors[0].key' <<<"$output")" = "thread:PRRT_a@42" ]
  [ "$(jq -r '.errors[0].recorded.commentId' <<<"$output")" = 1 ]
  # A later reviewer follow-up (new inReplyTo) is a new key: it reaches the
  # network layer instead — refused host proves the request was attempted.
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x PB_GH_BACKOFF='0 0 0' bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind thread --thread PRRT_a --root-comment 41 --in-reply-to 43 --body-file "$TMP/body.txt"
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].code' <<<"$doc")" = api_error ]
  [ "$(jq -r '.errors[0].source' <<<"$doc")" = reply ]
  # Nothing recorded on failure.
  [ "$(jq '.actions.replied | length' "$STATE")" = 1 ]
}

@test "reply: usage — kind/ids/body validated, empty body refused, untrusted body never in argv" {
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind thread --thread x --body-file "$TMP/body.txt"
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind review --body-file "$TMP/body.txt"
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind other --comment-id 1 --body-file "$TMP/body.txt"
  [ "$status" -eq 2 ]
  : >"$TMP/empty.txt"
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind conversation --comment-id 1 --body-file "$TMP/empty.txt"
  [ "$status" -eq 2 ]; [[ "$output" == *"reply body is empty"* ]]
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$STATE" --kind conversation --comment-id 1 --body-file "$TMP/nope.txt"
  [ "$status" -eq 2 ]
  ! grep -q -- '--body ' "$SCRIPTS/reply-thread.sh"
}

@test "resolve: a thread already recorded as confirmed returns 0 with no request" {
  jq '.actions.resolved["PRRT_b"] = {confirmed: true, at: "2026-09-19T12:01:00Z"}' "$STATE" >"$TMP/s.json" && mv "$TMP/s.json" "$STATE"
  out=$(env GH_HOST=127.0.0.1:1 GH_TOKEN=x bash "$SCRIPTS/resolve-thread.sh" --state-file "$STATE" --thread PRRT_b)
  [ "$(jq -r .alreadyResolved <<<"$out")" = true ]
  [ "$(jq -r .attempts <<<"$out")" = 0 ]
  # An unrecorded thread goes to the network; transport failure is api_error, nothing recorded.
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x PB_GH_BACKOFF='0 0 0' bash "$SCRIPTS/resolve-thread.sh" --state-file "$STATE" --thread PRRT_c
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].code' <<<"$doc")" = api_error ]
  [ "$(jq -r '.errors[0].source' <<<"$doc")" = resolve ]
  [ "$(jq -r '.actions.resolved.PRRT_c' "$STATE")" = null ]
}

# --- live (opt-in) -----------------------------------------------------------

live_write() {
  [ "${PR_BABYSIT_LIVE_WRITE:-}" = 1 ] || skip "set PR_BABYSIT_LIVE_WRITE=1 to create a throwaway PR on Falconiere/toolu (never in CI)"
  gh auth status >/dev/null 2>&1 || skip "gh not authenticated"
}

@test "AC-11 (live, opt-in): reply once, refuse the duplicate, resolve with confirmation, repeat resolve makes no request" {
  live_write
  repo=Falconiere/toolu
  branch="pr-babysit-write-side-test-$(date +%s)"
  work=$(mktemp -d)
  git clone -q --depth 1 "https://github.com/$repo.git" "$work/repo"
  cd "$work/repo"
  git config user.email hello@falconiere.io; git config user.name "pr-babysit write-side test"
  git checkout -q -b "$branch"
  printf 'throwaway line for the pr-babysit write-side live test\n' >>README.md
  git commit -qam "test(pr-babysit): throwaway write-side live test"
  git push -q origin "$branch"
  pr=$(gh pr create --repo "$repo" --head "$branch" --title "test(pr-babysit): throwaway write-side live test" --body "Created by write-side.bats; closed by its teardown." --draft --json number --jq .number 2>/dev/null \
       || gh pr view "$branch" --repo "$repo" --json number --jq .number)
  # One inline review thread on the changed line.
  gh api --method POST "repos/$repo/pulls/$pr/comments" -f body="Live test thread: please confirm." -f commit_id="$(git rev-parse HEAD)" -f path=README.md -F line="$(wc -l <README.md | tr -d ' ')" -f side=RIGHT >"$work/root.json"
  root=$(jq -r .id "$work/root.json")
  state="$TMP/pr-babysit-falconiere-toolu-$pr.json"
  bash "$SCRIPTS/babysit-tick.sh" --repo "$repo" --pr "$pr" --state-file "$state" >"$work/tick.json"
  thread=$(jq -r '.threads.actionable[0].id' "$work/tick.json")
  in_reply_to=$(jq -r '.threads.actionable[0].inReplyTo' "$work/tick.json")
  [ -n "$thread" ] && [ "$thread" != null ]
  [ "$in_reply_to" = "$root" ]
  before=$(gh api "repos/$repo/pulls/$pr/comments" --jq 'length')
  out=$(bash "$SCRIPTS/reply-thread.sh" --state-file "$state" --kind thread --thread "$thread" --root-comment "$root" --in-reply-to "$in_reply_to" --body-file "$TMP/body.txt")
  [ "$(jq -r .ok <<<"$out")" = true ]
  [ "$(gh api "repos/$repo/pulls/$pr/comments" --jq 'length')" = $((before + 1)) ]
  run bash "$SCRIPTS/reply-thread.sh" --state-file "$state" --kind thread --thread "$thread" --root-comment "$root" --in-reply-to "$in_reply_to" --body-file "$TMP/body.txt"
  [ "$status" -eq 4 ]
  [ "$(gh api "repos/$repo/pulls/$pr/comments" --jq 'length')" = $((before + 1)) ]
  out=$(bash "$SCRIPTS/resolve-thread.sh" --state-file "$state" --thread "$thread")
  [ "$(jq -r .confirmed <<<"$out")" = true ]
  [ "$(gh api graphql -F id="$thread" -f query='query($id:ID!){node(id:$id){... on PullRequestReviewThread{isResolved}}}' --jq '.data.node.isResolved')" = true ]
  out=$(env GH_HOST=127.0.0.1:1 GH_TOKEN=x bash "$SCRIPTS/resolve-thread.sh" --state-file "$state" --thread "$thread")
  [ "$(jq -r .alreadyResolved <<<"$out")" = true ]
  # Next tick: the thread is resolved, nothing actionable, nothing stale.
  bash "$SCRIPTS/babysit-tick.sh" --repo "$repo" --pr "$pr" --state-file "$state" >"$work/tick2.json"
  [ "$(jq '.threads.unresolved' "$work/tick2.json")" = 0 ]
  gh pr close "$pr" --repo "$repo" --delete-branch >/dev/null
  cd /; rm -rf "$work"
}
