#!/usr/bin/env bash
# reply-thread.sh — post one reply (review thread, conversation, or review-
# level) and record it in slot state so it is never posted twice.
#
# Usage: reply-thread.sh --state-file <path> --kind thread|conversation|review
#          (--thread <graphqlId> --root-comment <databaseId> --in-reply-to <databaseId>
#           | --comment-id <id> | --review-id <id>)
#          --body-file <path> [--timeout SECONDS]
#
# Idempotency key = the reviewer comment being answered:
#   thread:<threadId>@<inReplyTo> · conversation:<id> · review:<id>
# A key already present in actions.replied → exit 4 duplicate_reply, nothing
# posted. A later reviewer follow-up carries a new inReplyTo, so it is
# answerable. The body comes from a file so untrusted text never enters argv.
# The agent writes the words; this script makes the call land or says why not.
# Exit: 0 posted (stdout {ok,key,commentId,url}) · 2 usage · 3 api_error |
#       state_malformed · 4 duplicate_reply · 75 locked.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/lock.sh
. "$PB_SCRIPT_DIR/lib/lock.sh"
# shellcheck source=lib/gh.sh
. "$PB_SCRIPT_DIR/lib/gh.sh"
# shellcheck source=lib/state.sh
. "$PB_SCRIPT_DIR/lib/state.sh"

state_file=""; kind=""; thread=""; root=""; in_reply_to=""; comment_id=""; review_id=""; body_file=""
while [ $# -gt 0 ]; do
  case "$1" in
    --state-file)   state_file="${2:-}"; shift 2 ;;
    --kind)         kind="${2:-}"; shift 2 ;;
    --thread)       thread="${2:-}"; shift 2 ;;
    --root-comment) root="${2:-}"; shift 2 ;;
    --in-reply-to)  in_reply_to="${2:-}"; shift 2 ;;
    --comment-id)   comment_id="${2:-}"; shift 2 ;;
    --review-id)    review_id="${2:-}"; shift 2 ;;
    --body-file)    body_file="${2:-}"; shift 2 ;;
    --timeout)      PB_GH_TIMEOUT="${2:-}"; shift 2 ;;
    *) pb_fail usage "reply-thread.sh: unknown argument: $1" ;;
  esac
done
[ -n "$state_file" ] || pb_fail usage "reply-thread.sh: --state-file required"
[ -n "$body_file" ] && [ -f "$body_file" ] || pb_fail usage "reply-thread.sh: --body-file <existing file> required"
[ -s "$body_file" ] || pb_fail usage "reply-thread.sh: reply body is empty"
# GitHub caps comment bodies at 65536 characters; refuse before the request.
[ "$(wc -c <"$body_file" | tr -d ' ')" -le 65536 ] || pb_fail usage "reply-thread.sh: reply body exceeds GitHub's 65536-character limit"
case "$kind" in
  thread)
    [[ "$thread" =~ ^[A-Za-z0-9_=-]+$ ]] || pb_fail usage "reply-thread.sh: --thread <graphqlId> required for --kind thread"
    [[ "$root" =~ ^[0-9]+$ ]] || pb_fail usage "reply-thread.sh: --root-comment <databaseId> required for --kind thread"
    [[ "$in_reply_to" =~ ^[0-9]+$ ]] || pb_fail usage "reply-thread.sh: --in-reply-to <databaseId> required for --kind thread"
    key="thread:${thread}@${in_reply_to}" ;;
  conversation)
    [[ "$comment_id" =~ ^[0-9]+$ ]] || pb_fail usage "reply-thread.sh: --comment-id <id> required for --kind conversation"
    key="conversation:${comment_id}" ;;
  review)
    [[ "$review_id" =~ ^[0-9]+$ ]] || pb_fail usage "reply-thread.sh: --review-id <id> required for --kind review"
    key="review:${review_id}" ;;
  *) pb_fail usage "reply-thread.sh: --kind must be thread, conversation or review" ;;
esac

pb_require jq gh
pb_init
pb_mktmpdir
pb_lock_acquire "$state_file" || pb_lock_fail "$state_file"
pb_state_load "$state_file"

# Idempotency: refuse before any request.
if jq -e --arg k "$key" '.actions.replied[$k] != null' "$state_file" >/dev/null 2>&1; then
  pb_fail duplicate_reply "reply-thread.sh: a reply to $key is already recorded; not posting again" \
    "$(jq -c --arg k "$key" '{key:$k, recorded:.actions.replied[$k]}' "$state_file")"
fi

case "$kind" in
  thread)       endpoint="repos/$PB_STATE_REPO/pulls/$PB_STATE_NUMBER/comments/$root/replies" ;;
  conversation|review) endpoint="repos/$PB_STATE_REPO/issues/$PB_STATE_NUMBER/comments" ;;
esac
jq -n --rawfile body "$body_file" '{body:$body}' >"$PB_TMPDIR/payload.json"
pb_gh "$PB_TMPDIR/reply.json" api --method POST "$endpoint" --input "$PB_TMPDIR/payload.json" || pb_gh_fail reply
jq -e '.id | type == "number"' "$PB_TMPDIR/reply.json" >/dev/null 2>&1 \
  || pb_fail invalid_json "reply-thread.sh: reply response carried no comment id" '{"source":"reply"}'

pb_state_update "$state_file" \
  '.actions.replied[$k] = {commentId: $r.id, url: $r.html_url, at: $now, headSha: $head, kind: $kind}' \
  --arg k "$key" --arg now "$(pb_now)" --arg head "$PB_STATE_HEAD" --arg kind "$kind" \
  --argjson r "$(jq -c '{id, html_url}' "$PB_TMPDIR/reply.json")"
jq -c --arg k "$key" '{ok:true, key:$k, commentId:.id, url:.html_url}' "$PB_TMPDIR/reply.json"
