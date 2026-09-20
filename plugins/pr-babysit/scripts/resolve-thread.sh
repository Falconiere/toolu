#!/usr/bin/env bash
# resolve-thread.sh — resolve one review thread and CONFIRM it from the
# mutation response before recording it. A reply is never a resolve.
#
# Usage: resolve-thread.sh --state-file <path> --thread <graphqlId> [--timeout SECONDS]
#
# The mutation runs up to 3 times until the response shows
# thread.isResolved == true. Still false → exit 5 resolve_unconfirmed and the
# thread stays out of actions.resolved, so the next tick's resolution audit
# picks it up again. A thread already recorded as confirmed → exit 0 without
# any request. Transport failure after lib/gh.sh's bounded retry → exit 3.
# Exit: 0 confirmed (stdout {ok,thread,confirmed,attempts}) · 2 usage ·
#       3 api_error | state_malformed · 5 resolve_unconfirmed · 75 locked.
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

state_file=""; thread=""
while [ $# -gt 0 ]; do
  case "$1" in
    --state-file) state_file="${2:-}"; shift 2 ;;
    --thread)     thread="${2:-}"; shift 2 ;;
    --timeout)    PB_GH_TIMEOUT="${2:-}"; shift 2 ;;
    *) pb_fail usage "resolve-thread.sh: unknown argument: $1" ;;
  esac
done
[ -n "$state_file" ] || pb_fail usage "resolve-thread.sh: --state-file required"
[ -n "$thread" ] || pb_fail usage "resolve-thread.sh: --thread <graphqlId> required"

pb_require jq gh
pb_init
pb_mktmpdir
pb_lock_acquire "$state_file" || pb_lock_fail "$state_file"
pb_state_load "$state_file"

if jq -e --arg t "$thread" '.actions.resolved[$t].confirmed == true' "$state_file" >/dev/null 2>&1; then
  jq -c --arg t "$thread" '{ok:true, thread:$t, confirmed:true, attempts:0, alreadyResolved:true, at:.actions.resolved[$t].at}' "$state_file"
  exit 0
fi

mutation='mutation($threadId:ID!){ resolveReviewThread(input:{threadId:$threadId}){ thread{ id isResolved } } }'
attempt=0; confirmed=false; last=""
while [ "$attempt" -lt 3 ]; do
  attempt=$((attempt + 1))
  pb_gh "$PB_TMPDIR/resolve.json" api graphql -F threadId="$thread" -f query="$mutation" || pb_gh_fail resolve
  pb_gh_json_ok "$PB_TMPDIR/resolve.json" || pb_fail invalid_json "resolve-thread.sh: mutation response carried errors[]" \
    "$(jq -c '{source:"resolve", errors:(.errors // [])}' "$PB_TMPDIR/resolve.json")"
  last=$(jq -c '.data.resolveReviewThread.thread // null' "$PB_TMPDIR/resolve.json")
  if [ "$(jq -r '.isResolved // false' <<<"$last")" = true ]; then confirmed=true; break; fi
  echo "resolve-thread.sh: attempt $attempt returned isResolved=false for $thread; retrying" >&2
  sleep 1
done

if [ "$confirmed" != true ]; then
  pb_fail resolve_unconfirmed "resolve-thread.sh: $thread still unresolved after $attempt attempt(s)" \
    "$(jq -nc --arg t "$thread" --argjson attempts "$attempt" --argjson last "${last:-null}" '{thread:$t, attempts:$attempts, lastResponse:$last}')"
fi

pb_state_update "$state_file" \
  '.actions.resolved[$t] = {confirmed: true, at: $now, attempts: $attempts, headSha: $head}' \
  --arg t "$thread" --arg now "$(pb_now)" --argjson attempts "$attempt" --arg head "$PB_STATE_HEAD"
jq -nc --arg t "$thread" --argjson attempts "$attempt" '{ok:true, thread:$t, confirmed:true, attempts:$attempts}'
