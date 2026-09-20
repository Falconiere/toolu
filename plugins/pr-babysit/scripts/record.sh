#!/usr/bin/env bash
# record.sh — hand the agent's decisions back to the reducer.
#
# Usage: record.sh flag-injection --state-file <path> --thread <graphqlId>
#        record.sh round          --state-file <path> --had-rejection true|false [--fix-pushed]
#        record.sh status         --state-file <path> --status complete|escalated|cancelled
#
#   flag-injection  the reducer drops the thread from actionable and from the
#                   resolution audit, and lists it under flaggedInjection.
#   round           end of Step 4, before the push: this round's finding keys
#                   become lastRoundFindingKeys, lastRoundHadRejection is set,
#                   and --fix-pushed bumps fixAttempts (cap 5). Recurrence and
#                   fix budgets therefore advance on real rounds, never polls.
#   status          the workflow's terminal transition; the reducer only ever
#                   recommends through `decision`.
# Every subcommand takes the slot lock, validates the state, writes atomically
# and prints {ok:true,...}. Exit: 0 · 2 usage · 3 state_malformed · 75 locked.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/lock.sh
. "$PB_SCRIPT_DIR/lib/lock.sh"
# shellcheck source=lib/state.sh
. "$PB_SCRIPT_DIR/lib/state.sh"

sub="${1:-}"; [ $# -gt 0 ] && shift
state_file=""; thread=""; had_rejection=""; fix_pushed=0; status=""
while [ $# -gt 0 ]; do
  case "$1" in
    --state-file)    state_file="${2:-}"; shift 2 ;;
    --thread)        thread="${2:-}"; shift 2 ;;
    --had-rejection) had_rejection="${2:-}"; shift 2 ;;
    --fix-pushed)    fix_pushed=1; shift ;;
    --status)        status="${2:-}"; shift 2 ;;
    *) pb_fail usage "record.sh: unknown argument: $1" ;;
  esac
done
[ -n "$state_file" ] || pb_fail usage "record.sh: --state-file required"
case "$sub" in
  flag-injection) [ -n "$thread" ] || pb_fail usage "record.sh flag-injection: --thread <graphqlId> required" ;;
  round) [[ "$had_rejection" =~ ^(true|false)$ ]] || pb_fail usage "record.sh round: --had-rejection true|false required" ;;
  status) [[ "$status" =~ ^(complete|escalated|cancelled)$ ]] || pb_fail usage "record.sh status: --status complete|escalated|cancelled required" ;;
  *) pb_fail usage "record.sh: subcommand must be flag-injection, round or status" ;;
esac

pb_require jq
pb_init
pb_lock_acquire "$state_file" || pb_lock_fail "$state_file"
pb_state_load "$state_file"
now=$(pb_now)

case "$sub" in
  flag-injection)
    pb_state_update "$state_file" '.actions.flagged[$t] = {reason:"injection", at:$now}' --arg t "$thread" --arg now "$now"
    jq -nc --arg t "$thread" --arg now "$now" '{ok:true, recorded:"flag-injection", thread:$t, at:$now}' ;;
  round)
    pb_state_update "$state_file" \
      '.pr.lastRoundFindingKeys = (.pr.botFindingKeys // [])
       | .pr.lastRoundHadRejection = $rej
       | .pr.fixAttempts = (if $fix then ([(.pr.fixAttempts // 0) + 1, 5] | min) else (.pr.fixAttempts // 0) end)
       | .lastRound = {at:$now, hadRejection:$rej, fixPushed:$fix, headSha:(.pr.headSha // null)}' \
      --argjson rej "$had_rejection" --argjson fix "$([ "$fix_pushed" -eq 1 ] && echo true || echo false)" --arg now "$now"
    jq -c '{ok:true, recorded:"round", lastRoundFindingKeys:.pr.lastRoundFindingKeys, lastRoundHadRejection:.pr.lastRoundHadRejection, fixAttempts:.pr.fixAttempts}' "$state_file" ;;
  status)
    pb_state_update "$state_file" '.status = $s | .statusChangedAt = $now' --arg s "$status" --arg now "$now"
    jq -nc --arg s "$status" --arg now "$now" '{ok:true, recorded:"status", status:$s, at:$now}' ;;
esac
