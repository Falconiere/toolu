#!/usr/bin/env bash
# collect-pr.sh — the read side of the pr-babysit helper.
#
# Fetches everything one babysit tick needs from GitHub and writes ONE
# versioned snapshot. No decisions are made here (see reduce-state.sh); the
# only judgment is "which comment is the CI reviewer's verdict", delegated to
# parse-verdict.sh.
#
# Usage: collect-pr.sh --repo <owner/repo> --pr <n> --out <path>
#                      [--page-size N] [--timeout SECONDS]
#
# Behavior (spec: docs/toolu/specs/2026-09-19-pr-babysit-helper-design.md):
#  - four independent reads run concurrently as background jobs (pr view,
#    review threads, issue comments, reviews), each through lib/gh.sh's
#    timeout + bounded retry; any failure aborts the whole collection with a
#    structured error and NO snapshot;
#  - reviewThreads paginate via `gh api graphql --paginate --slurp`; a
#    thread whose comments overflow one page is completed with a cursor loop
#    on node(id:); REST comments/reviews paginate via --paginate --slurp;
#  - the PR head is read before and after the fan-out; if it moved the whole
#    collection runs again once, then fails with head_moved;
#  - the bot comment is the LAST CI-reviewer issue comment that
#    parse-verdict.sh recognises as a review, else the last CI-reviewer
#    comment (verdict state unknown), else null (state absent).
# Exit: 0 snapshot written · 2 usage · 3 structured error (stdout JSON).
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/gh.sh
. "$PB_SCRIPT_DIR/lib/gh.sh"
# shellcheck source=lib/normalize.sh
. "$PB_SCRIPT_DIR/lib/normalize.sh"

repo=""; number=""; out=""; page_size=100
while [ $# -gt 0 ]; do
  case "$1" in
    --repo)      repo="${2:-}"; shift 2 ;;
    --pr)        number="${2:-}"; shift 2 ;;
    --out)       out="${2:-}"; shift 2 ;;
    --page-size) page_size="${2:-}"; shift 2 ;;
    --timeout)   PB_GH_TIMEOUT="${2:-}"; shift 2 ;;
    *) pb_fail usage "collect-pr.sh: unknown argument: $1" ;;
  esac
done
[[ "$repo" =~ ^[^/[:space:]]+/[^/[:space:]]+$ ]] || pb_fail usage "collect-pr.sh: --repo <owner/repo> required"
[[ "$number" =~ ^[0-9]+$ ]] || pb_fail usage "collect-pr.sh: --pr <n> required"
[ -n "$out" ] || pb_fail usage "collect-pr.sh: --out <path> required"
[[ "$page_size" =~ ^[0-9]+$ ]] && [ "$page_size" -ge 1 ] || pb_fail usage "collect-pr.sh: --page-size must be a positive integer"
[[ "$PB_GH_TIMEOUT" =~ ^[0-9]+$ ]] && [ "$PB_GH_TIMEOUT" -ge 1 ] || pb_fail usage "collect-pr.sh: --timeout must be a positive integer"

pb_require jq gh
pb_init
pb_mktmpdir
owner="${repo%%/*}"; name="${repo#*/}"
PARSER="$(pb_plugin_root)/scripts/parse-verdict.sh"
[ -f "$PARSER" ] || pb_fail usage "collect-pr.sh: parse-verdict.sh not found at $PARSER"

# _read_head -> current headRefOid, or exits through pb_gh_fail.
_read_head() {
  pb_gh "$PB_TMPDIR/head.json" pr view "$number" --repo "$repo" --json headRefOid || pb_gh_fail head
  jq -er '.headRefOid' "$PB_TMPDIR/head.json" 2>/dev/null || pb_fail invalid_json "collect-pr.sh: head read returned no headRefOid" '{"source":"head"}'
}

# _job NAME GH_ARGS... — one background read. Writes NAME.json and NAME.meta
# (the pb_gh outcome) so the parent can report exactly which read failed.
_job() {
  local jname="$1"; shift
  local rc=0
  pb_gh "$PB_TMPDIR/$jname.json" "$@" || rc=$?
  if [ "$rc" -eq 0 ] && ! pb_gh_json_ok "$PB_TMPDIR/$jname.json"; then
    rc=1; PB_GH_CLASS=invalid_json
    PB_GH_LAST_ERR="response is not valid JSON or carries GraphQL errors[]"
  fi
  jq -nc --argjson rc "$rc" --argjson attempts "${PB_GH_ATTEMPTED:-0}" \
    --arg class "${PB_GH_CLASS:-}" --arg lastMessage "${PB_GH_LAST_ERR:-}" \
    '{rc:$rc, attempts:$attempts, class:$class, lastMessage:$lastMessage}' >"$PB_TMPDIR/$jname.meta"
}

# _fan_out — run the four reads concurrently, then fail on the first bad meta.
_fan_out() {
  local j pids=""
  _job pr pr view "$number" --repo "$repo" --json "$(pb_pr_view_fields)" & pids="$pids $!"
  # Strings go through -f (raw); -F would coerce a numeric-looking owner or
  # repo name into a JSON number and break the GraphQL variable types.
  _job threads api graphql --paginate --slurp \
    -f owner="$owner" -f repo="$name" -F number="$number" -F pageSize="$page_size" \
    -f query="$(pb_gql_threads)" & pids="$pids $!"
  _job comments api --paginate --slurp "repos/$repo/issues/$number/comments?per_page=$page_size" & pids="$pids $!"
  _job reviews api --paginate --slurp "repos/$repo/pulls/$number/reviews?per_page=$page_size" & pids="$pids $!"
  for j in $pids; do wait "$j" || true; done
  for j in pr threads comments reviews; do
    [ -f "$PB_TMPDIR/$j.meta" ] || pb_fail api_error "collect-pr.sh: read '$j' produced no outcome" "{\"source\":\"$j\"}"
    if [ "$(jq -r .rc "$PB_TMPDIR/$j.meta")" != 0 ]; then
      local code=api_error
      [ "$(jq -r .class "$PB_TMPDIR/$j.meta")" = invalid_json ] && code=invalid_json
      pb_fail "$code" "collect-pr.sh: read '$j' failed: $(jq -r .lastMessage "$PB_TMPDIR/$j.meta")" \
        "$(jq -c --arg source "$j" '{source:$source, attempts, class, lastMessage}' "$PB_TMPDIR/$j.meta")"
    fi
  done
}

# _complete_thread_comments — for every thread whose comment page overflowed,
# follow the cursor on node(id:) and splice the remaining comments in.
_complete_thread_comments() {
  local ids tid extra=0
  ids=$(jq -r '.[] | select(.commentsHasNextPage == true) | .id' "$PB_TMPDIR/threads.norm.json")
  [ -n "$ids" ] || { echo 0 >"$PB_TMPDIR/threadComments.pages"; return 0; }
  for tid in $ids; do
    local cursor
    cursor=$(jq -r --arg id "$tid" '.[] | select(.id == $id) | .commentsEndCursor' "$PB_TMPDIR/threads.norm.json")
    # The first page is already in hand: resume from its endCursor.
    pb_gh "$PB_TMPDIR/tc.json" api graphql --paginate --slurp \
      -f id="$tid" -F pageSize="$page_size" -f endCursor="$cursor" \
      -f query="$(pb_gql_thread_comments)" || pb_gh_fail threadComments
    pb_gh_json_ok "$PB_TMPDIR/tc.json" || pb_fail invalid_json "collect-pr.sh: thread comment page for $tid is not valid JSON" '{"source":"threadComments"}'
    extra=$((extra + $(jq 'length' "$PB_TMPDIR/tc.json")))
    jq "$(pb_jq_thread_comments_from_pages)" "$PB_TMPDIR/tc.json" >"$PB_TMPDIR/tc.norm.json"
    jq --arg id "$tid" --slurpfile more "$PB_TMPDIR/tc.norm.json" \
      'map(if .id == $id then .comments += $more[0] | .commentsHasNextPage = false | .commentsEndCursor = null else . end)' \
      "$PB_TMPDIR/threads.norm.json" >"$PB_TMPDIR/threads.norm.next.json"
    mv "$PB_TMPDIR/threads.norm.next.json" "$PB_TMPDIR/threads.norm.json"
  done
  echo "$extra" >"$PB_TMPDIR/threadComments.pages"
}

# _select_bot_comment — write bot.json: {comment, verdict}.
_select_bot_comment() {
  local ids cid body chosen=""
  ids=$(jq -r "$(pb_jq_defs) [.[] | select(.author | is_ci_reviewer)] | reverse | .[].id" "$PB_TMPDIR/comments.norm.json")
  for cid in $ids; do
    body=$(jq -r --argjson id "$cid" '.[] | select(.id == $id) | .body' "$PB_TMPDIR/comments.norm.json")
    printf '%s' "$body" | bash "$PARSER" >"$PB_TMPDIR/verdict.json"
    if jq -e '.is_review_comment == true' "$PB_TMPDIR/verdict.json" >/dev/null; then chosen="$cid"; break; fi
  done
  if [ -z "$chosen" ]; then
    # No recognised review: fall back to the last CI-reviewer comment (state
    # unknown, degraded) or none at all (state absent).
    chosen=$(printf '%s\n' "$ids" | head -n 1)
    if [ -n "$chosen" ]; then
      body=$(jq -r --argjson id "$chosen" '.[] | select(.id == $id) | .body' "$PB_TMPDIR/comments.norm.json")
      printf '%s' "$body" | bash "$PARSER" >"$PB_TMPDIR/verdict.json"
    else
      printf '' | bash "$PARSER" | jq '.state = "absent"' >"$PB_TMPDIR/verdict.json"
    fi
  fi
  if [ -n "$chosen" ]; then
    jq -c --argjson id "$chosen" --slurpfile v "$PB_TMPDIR/verdict.json" \
      '{comment: (.[] | select(.id == $id) | {id, url, createdAt, updatedAt, author}), verdict: $v[0]}' \
      "$PB_TMPDIR/comments.norm.json" >"$PB_TMPDIR/bot.json"
  else
    jq -nc --slurpfile v "$PB_TMPDIR/verdict.json" '{comment: null, verdict: $v[0]}' >"$PB_TMPDIR/bot.json"
  fi
}

_collect_once() {
  local head_before head_after
  head_before=$(_read_head)
  _fan_out
  jq "$(pb_jq_threads_from_pages)" "$PB_TMPDIR/threads.json" >"$PB_TMPDIR/threads.norm.json"
  _complete_thread_comments
  jq "$(pb_jq_rest_items) | map($(pb_jq_issue_comment))" "$PB_TMPDIR/comments.json" >"$PB_TMPDIR/comments.norm.json"
  jq "$(pb_jq_rest_items) | map($(pb_jq_review))" "$PB_TMPDIR/reviews.json" >"$PB_TMPDIR/reviews.norm.json"
  _select_bot_comment
  head_after=$(_read_head)
  [ "$head_before" = "$head_after" ] || return 10
  printf '%s' "$head_after" >"$PB_TMPDIR/head.sha"
}

moved=0
if ! _collect_once; then
  moved=1
  echo "pr-babysit: PR head moved during collection; collecting again" >&2
  _collect_once || pb_fail head_moved "collect-pr.sh: PR head moved twice during collection" '{"source":"head"}'
fi

_build_snapshot() {
  jq -n \
    --arg collectedAt "$(pb_now)" --arg repo "$repo" --argjson number "$number" \
    --arg head "$(cat "$PB_TMPDIR/head.sha")" --argjson recollected "$moved" \
    --argjson pageSize "$page_size" \
    --slurpfile pr "$PB_TMPDIR/pr.json" \
    --slurpfile threads "$PB_TMPDIR/threads.norm.json" \
    --slurpfile comments "$PB_TMPDIR/comments.norm.json" \
    --slurpfile reviews "$PB_TMPDIR/reviews.norm.json" \
    --slurpfile bot "$PB_TMPDIR/bot.json" \
    --argjson pThreads "$(jq 'length' "$PB_TMPDIR/threads.json")" \
    --argjson pThreadComments "$(cat "$PB_TMPDIR/threadComments.pages")" \
    --argjson pComments "$(jq 'length' "$PB_TMPDIR/comments.json")" \
    --argjson pReviews "$(jq 'length' "$PB_TMPDIR/reviews.json")" \
    '{version: 1, collectedAt: $collectedAt, repo: $repo, number: $number,
      head: {sha: $head, verifiedAfterFanout: true, recollected: ($recollected == 1)},
      pageSize: $pageSize,
      pr: ($pr[0] | {number, title, url, author: (.author.login // null), state, baseRefName, headRefName,
                     headRefOid, mergeable, reviewDecision, statusCheckRollup: (.statusCheckRollup // [])}),
      threads: ($threads[0] | map(del(.commentsHasNextPage, .commentsEndCursor))),
      comments: $comments[0], reviews: $reviews[0], bot: $bot[0],
      pages: {threads: $pThreads, threadComments: $pThreadComments, comments: $pComments, reviews: $pReviews}}'
}
pb_atomic_write_json "$out" _build_snapshot || pb_fail invalid_json "collect-pr.sh: could not assemble the snapshot" '{"source":"snapshot"}'
printf '%s\n' "$out"
