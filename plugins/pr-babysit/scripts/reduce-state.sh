#!/usr/bin/env bash
# reduce-state.sh — the pure decision layer of the pr-babysit helper.
#
# Reads one snapshot (collect-pr.sh output) and the previous slot state, and
# emits the next state plus the compact agent-facing result. No network, no
# clock (--now is an input), no side effects beyond the two output files, so
# every decision rule below is testable against captured real snapshots.
#
# Usage: reduce-state.sh --snapshot <path> --state <path> --now <iso8601>
#                        [--state-out <path>] [--result-out <path>]
#                        [--state-path <path>] [--snapshot-path <path>]
#   --state         previous state file; absent file = first tick
#   --state-out     where to write the next state (skipped when omitted)
#   --result-out    where to write the result (stdout when omitted)
#   --state-path / --snapshot-path  the paths reported inside the result
#
# The rules encoded here are the workflow's (workflows/babysit.md): the Step 1
# actionable filter and resolution audit, the Step 4 recurrence gate and the
# Step 6 stop conditions. Reason and error codes are closed sets — see the spec
# docs/toolu/specs/2026-09-19-pr-babysit-helper-design.md.
# Exit: 0 result produced · 2 usage · 3 state_malformed | slot_mismatch | invalid_json.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/normalize.sh
. "$PB_SCRIPT_DIR/lib/normalize.sh"

snapshot=""; state=""; now=""; state_out=""; result_out=""; state_path=""; snapshot_path=""
while [ $# -gt 0 ]; do
  case "$1" in
    --snapshot)      snapshot="${2:-}"; shift 2 ;;
    --state)         state="${2:-}"; shift 2 ;;
    --now)           now="${2:-}"; shift 2 ;;
    --state-out)     state_out="${2:-}"; shift 2 ;;
    --result-out)    result_out="${2:-}"; shift 2 ;;
    --state-path)    state_path="${2:-}"; shift 2 ;;
    --snapshot-path) snapshot_path="${2:-}"; shift 2 ;;
    *) pb_fail usage "reduce-state.sh: unknown argument: $1" ;;
  esac
done
[ -n "$snapshot" ] || pb_fail usage "reduce-state.sh: --snapshot <path> required"
[ -n "$state" ] || pb_fail usage "reduce-state.sh: --state <path> required"
[[ "$now" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || pb_fail usage "reduce-state.sh: --now must be an ISO-8601 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)"
[ -n "$state_path" ] || state_path="$state"
[ -n "$snapshot_path" ] || snapshot_path="$snapshot"
pb_require jq

# --- validate inputs (fail closed, never write) ------------------------------
[ -f "$snapshot" ] || pb_fail invalid_json "reduce-state.sh: snapshot not found: $snapshot" '{"source":"snapshot"}'
pb_json_valid "$snapshot" || pb_fail invalid_json "reduce-state.sh: snapshot is not valid JSON: $snapshot" '{"source":"snapshot"}'
jq -e '.version == 1 and (.repo | type == "string") and (.number | type == "number") and (.head.sha | type == "string") and (.pr | type == "object") and (.threads | type == "array")' "$snapshot" >/dev/null 2>&1 \
  || pb_fail invalid_json "reduce-state.sh: snapshot is not a version-1 pr-babysit snapshot" '{"source":"snapshot"}'

prev_json="null"
if [ -e "$state" ]; then
  pb_json_valid "$state" || pb_fail state_malformed "reduce-state.sh: state file is not valid JSON: $state" '{"source":"state"}'
  jq -e '.version == 2' "$state" >/dev/null 2>&1 \
    || pb_fail state_malformed "reduce-state.sh: state file is not version 2: $state" "$(jq -c '{source:"state", version: (.version // null)}' "$state")"
  if ! jq -e --slurpfile s "$snapshot" '.repo == $s[0].repo and .number == $s[0].number' "$state" >/dev/null 2>&1; then
    pb_fail slot_mismatch "reduce-state.sh: state belongs to $(jq -r '"\(.repo)#\(.number)"' "$state"), snapshot is $(jq -r '"\(.repo)#\(.number)"' "$snapshot")" \
      "$(jq -c --slurpfile s "$snapshot" '{source:"state", state:{repo, number}, snapshot:{repo:$s[0].repo, number:$s[0].number}}' "$state")"
  fi
  prev_json=$(cat "$state")
fi

# --- the reducer -------------------------------------------------------------
pb_jq_reduce() {
  pb_jq_defs
  cat <<'EOF'
# ---- injection heuristics (advisory only): the agent keeps the decision.
def injection_pattern:
  . as $b
  | [ "ignore (all |any |the )?(previous|prior|above|earlier) (instructions|prompts?|rules)",
      "disregard (all |any |the |your )?(previous|prior|system|above) ",
      "you are (now )?(an? )?(ai|assistant|llm|language model|claude|codex|copilot)",
      "(^|\\n)\\s*(system|assistant)\\s*:",
      "<(system|instructions?)>",
      "(run|execute) (the following|this|these) (command|shell|script)" ]
  | map(. as $p | select($b | test($p; "i"))) | first // null;
# author_class: ci_reviewer (by exact login) | bot (GraphQL Bot / REST type) | human.
def author_class: if (.author | is_ci_reviewer) then "ci_reviewer" elif .authorType == "Bot" then "bot" else "human" end;
# ---- inputs
$snap as $snap | $prev as $prev
| ($snap.pr) as $pr
| ($snap.head.sha) as $head
| ($pr.author) as $author
| (($snap.repo | ascii_downcase | gsub("/"; "-")) + "-" + ($snap.number | tostring)) as $slot
| ($snap.repo + "#" + ($snap.number | tostring)) as $key
| (if $prev == null then {replied:{}, resolved:{}, flagged:{}} else ($prev.actions // {replied:{}, resolved:{}, flagged:{}}) end) as $actions
| ($actions.flagged | keys) as $flagged
| ($actions.replied | keys) as $repliedKeys
# ---- CI
| ($pr.statusCheckRollup // []) as $rollup
| ($rollup | ci_status) as $ciStatus
| ($rollup | map({name: check_name, status: check_state, url: check_url})) as $checks
# ---- verdict
| ($snap.bot.verdict // {}) as $v
| ($snap.bot.comment) as $botComment
| ($v.state // "absent") as $botState
| ($v.verdict // "none") as $botVerdict
| (($v.findings // []) | map(.key)) as $keys
| (($v.findings // []) | length) as $findingsCount
| ($botState == "absent" or $botState == "unknown" or ($v.is_review_comment == false)) as $degraded
| (if $botState == "absent" then "review_absent" elif $degraded then "review_unknown_format" else null end) as $degradedReason
| ($prev != null and $botComment != null and ($prev.pr.botCommentId // null) == $botComment.id and ($prev.pr.botCommentUpdatedAt // null) == $botComment.updatedAt) as $sameRun
# ---- thread classification
| ($snap.threads | map(
    . as $t
    | ($t.comments | last) as $lastComment
    | ($t.comments | map(select(.author != $author)) | last) as $lastNonAuthor
    | ($flagged | index($t.id) != null) as $isFlagged
    | (if $lastNonAuthor == null then "none" else ($lastNonAuthor | author_class) end) as $class
    | ($t.isResolved == false) as $open
    | ($open and ($isFlagged | not) and $lastComment != null and $lastComment.author != $author and ($class == "human" or $class == "ci_reviewer")) as $answerable
    | (if $t.isOutdated then ($answerable and $class == "human") else $answerable end) as $actionable
    | ($open and ($t.isOutdated | not) and ($isFlagged | not)) as $audited
    | { id: $t.id, path: $t.path, line: $t.line, isOutdated: $t.isOutdated, isResolved: $t.isResolved,
        rootCommentId: ($t.comments | first | .databaseId // null),
        inReplyTo: ($lastNonAuthor.databaseId // null),
        authorClass: $class,
        lastCommentAuthor: ($lastComment.author // null),
        lastCommentAt: ($lastComment.createdAt // null),
        injectionSuspect: ((($lastNonAuthor.body // "") | injection_pattern) != null),
        injectionPattern: (($lastNonAuthor.body // "") | injection_pattern),
        comments: $t.comments,
        flags: {actionable: $actionable, audited: $audited, flagged: $isFlagged,
                skippedOutdated: ($open and $t.isOutdated and $class == "ci_reviewer"),
                replied: ($repliedKeys | index("thread:" + $t.id + "@" + (($lastNonAuthor.databaseId // 0) | tostring)) != null)} }
  )) as $threads
| ($threads | map(select(.flags.actionable)) | map(del(.flags, .isResolved))) as $actionable
| ($threads | map(select(.flags.audited))) as $audited
| ($audited | map(select(.flags.actionable | not)) | map({id, path, line, repliedAt: .lastCommentAt, lastCommentAuthor})) as $staleUnresolved
| ($threads | map(select(.flags.skippedOutdated)) | map(.id)) as $skippedOutdated
| ($threads | map(select(.flags.flagged)) | map(.id)) as $flaggedInjection
| ($audited | length) as $unresolved
# ---- conversation and review-level comments
| (($snap.comments // []) | map(select(.author != $author and .authorType != "Bot"))
    | map(. as $c | select(($snap.comments | map(select(.author == $author and .createdAt > $c.createdAt)) | length) == 0))
    | map(. as $c | select(($repliedKeys | index("conversation:" + ($c.id | tostring))) == null))
    | map({id, author, body, createdAt, url})) as $convActionable
| (($snap.reviews // []) | map(select(.author != $author and .authorType != "Bot" and .state != "APPROVED" and ((.body // "") | length) > 0))
    | map(. as $r | select(($repliedKeys | index("review:" + ($r.id | tostring))) == null))
    | map({id, author, state, body, submittedAt, url})) as $reviewActionable
# ---- recurrence (Step 4 gate) — only a NEW verdict run can recur
| (if $prev == null then [] else ($prev.pr.lastRoundFindingKeys // []) end) as $lastRoundKeys
| (if $prev == null then false else ($prev.pr.lastRoundHadRejection // false) end) as $lastRoundHadRejection
| (if $prev == null then 0 else ($prev.pr.recurrenceStreak // 0) end) as $prevStreak
| (if $prev == null then 0 else ($prev.pr.fixAttempts // 0) end) as $fixAttempts
| (if $sameRun or $prev == null then [] else ($keys | map(select(. as $k | $lastRoundKeys | index($k) != null))) end) as $recurringKeys
| (if $sameRun then $prevStreak elif ($recurringKeys | length) > 0 then ($prevStreak + 1) else 0 end) as $streak
# ---- change detection + backoff
| ({ciStatus: $ciStatus, reviewDecision: $pr.reviewDecision, mergeable: $pr.mergeable, unresolvedThreads: $unresolved,
    headSha: $head, botVerdict: $botVerdict, botState: $botState, botFindingKeys: $keys}) as $cmp
| ($prev == null or ($prev.pr | {ciStatus, reviewDecision, mergeable, unresolvedThreads, headSha, botVerdict, botState, botFindingKeys}) != $cmp) as $changed
| (if $changed then 0 else (($prev.idleStreak // 0) + 1) end) as $idleStreak
| (if $idleStreak >= 9 then 15 elif $idleStreak >= 6 then 12 elif $idleStreak >= 3 then 6 elif $ciStatus == "fail" then 1 else 3 end) as $intervalMinutes
| (if $idleStreak >= 6 then 60 elif $idleStreak >= 3 then 30 else 15 end) as $waitSeconds
# ---- decision (Step 6)
| ($prev != null and $botState == "provider_error" and ($prev.pr.botState // "") == "provider_error" and ($prev.pr.headSha // "") == $head) as $providerErrorRepeated
| ([ (if $pr.state == "MERGED" then {code:"pr_merged", detail:"PR is merged"} else empty end),
     (if $pr.state == "CLOSED" then {code:"pr_closed", detail:"PR is closed"} else empty end),
     (if $pr.mergeable == "CONFLICTING" then {code:"merge_conflict", detail:"mergeable is CONFLICTING"} else empty end),
     (if $fixAttempts >= 5 then {code:"fix_attempts_exhausted", detail:"\($fixAttempts) fix attempts recorded"} else empty end),
     (if ($recurringKeys | length) > 0 and $lastRoundHadRejection then {code:"recurrence_after_rejection", detail:"\($recurringKeys | length) finding key(s) recurred after a Won't-fix round"} else empty end),
     (if ($recurringKeys | length) > 0 and $streak >= 2 then {code:"recurrence_streak", detail:"finding keys recurred on \($streak) consecutive rounds"} else empty end),
     (if $providerErrorRepeated then {code:"provider_error_repeated", detail:"review provider error twice on head \($head[0:8])"} else empty end)
   ]) as $escalations
| ([ (if $ciStatus == "pass" then {code:"ci_pass", detail:"\($checks | length) check(s) passed"}
      elif $ciStatus == "fail" then {code:"ci_failed", detail:($checks | map(select(.status == "fail")) | map(.name) | join(", "))}
      else {code:"ci_pending", detail:(if ($checks | length) == 0 then "no checks reported yet" else ($checks | map(select(.status == "pending")) | map(.name) | join(", ")) end)} end),
     (if $unresolved == 0 then {code:"threads_clear", detail:"no unresolved review threads"} else empty end),
     (if ($actionable | length) > 0 then {code:"threads_unresolved", detail:"\($actionable | length) actionable thread(s)"} else empty end),
     (if ($staleUnresolved | length) > 0 then {code:"threads_stale_unresolved", detail:"\($staleUnresolved | length) replied-but-unresolved thread(s)"} else empty end),
     (if $botState == "in_progress" then {code:"review_in_progress", detail:"review bot still running"}
      elif $botState == "provider_error" and ($providerErrorRepeated | not) then {code:"provider_error", detail:"review bot reported a provider error; rerun the review job once"}
      elif $botState == "complete" and $botVerdict == "approved" and $findingsCount == 0 then {code:"review_approved", detail:"bot verdict approved with zero findings"}
      elif $botState == "complete" then {code:"review_changes", detail:"bot verdict \($botVerdict) with \($findingsCount) finding(s)"}
      elif $degraded then {code:$degradedReason, detail:"bot verdict cannot be read"} else empty end),
     (if $degraded then {code:"manual_verify", detail:"verify review findings manually: \($botComment.url // "no bot comment")"} else empty end),
     (if $pr.mergeable == "UNKNOWN" and $pr.state == "OPEN" then {code:"mergeable_unknown", detail:"GitHub has not computed mergeability yet"} else empty end),
     (if $changed then empty else {code:"unchanged", detail:"nothing changed since the last tick"} end)
   ]) as $signals
| (($escalations | length) > 0) as $escalate
| ($pr.state == "OPEN" and $ciStatus == "pass" and $unresolved == 0 and $pr.mergeable != "UNKNOWN"
   and ( ($botState == "complete" and $botVerdict == "approved" and $findingsCount == 0) or $degraded )) as $successReady
| (if $escalate then "escalate" elif $successReady then "success" else "keep_going" end) as $decision
| ($escalations + $signals) as $reasons
# ---- outputs
| {
  state: {
    version: 2, slot: $slot, repo: $snap.repo, number: $snap.number,
    cronName: ("pr-babysit:" + $slot),
    lastUpdate: $now,
    totalTicks: ((if $prev == null then 0 else ($prev.totalTicks // 0) end) + 1),
    idleStreak: $idleStreak, currentInterval: $intervalMinutes, waitSeconds: $waitSeconds,
    status: (if $prev == null then "active" else ($prev.status // "active") end),
    worktree: (if $prev == null then null else ($prev.worktree // null) end),
    pr: {
      key: $key, ciStatus: $ciStatus, reviewDecision: $pr.reviewDecision, mergeable: $pr.mergeable,
      unresolvedThreads: $unresolved, headSha: $head, fixAttempts: $fixAttempts,
      botVerdict: $botVerdict, botState: $botState,
      botCommentId: ($botComment.id // null), botCommentUpdatedAt: ($botComment.updatedAt // null),
      botFindingKeys: $keys, lastRoundFindingKeys: $lastRoundKeys, lastRoundHadRejection: $lastRoundHadRejection,
      recurrenceStreak: $streak, unresolvedAfterClearance: $unresolved, lastError: null
    },
    actions: $actions,
    lastGoodSnapshot: $snapshotPath
  },
  result: {
    version: 1, slot: $slot, changed: $changed, decision: $decision, reasons: $reasons,
    pr: {number: $pr.number, url: $pr.url, head: $head, branch: $pr.headRefName, base: $pr.baseRefName, author: $author,
         state: $pr.state, mergeable: $pr.mergeable, reviewDecision: $pr.reviewDecision},
    ci: {status: $ciStatus, checks: $checks},
    verdict: {state: $botState, verdict: $botVerdict, findingsCount: $findingsCount, findingKeys: $keys,
              mustFix: ($v.must_fix // []), commentUrl: ($botComment.url // null), commentId: ($botComment.id // null),
              degraded: $degraded, degradedReason: $degradedReason, sameRunAsLastTick: $sameRun},
    threads: {total: ($snap.threads | length), unresolved: $unresolved,
              actionable: $actionable, staleUnresolved: $staleUnresolved,
              skippedOutdated: $skippedOutdated, flaggedInjection: $flaggedInjection},
    conversation: {actionable: $convActionable},
    reviews: {actionable: $reviewActionable},
    recurrence: {streak: $streak, lastRoundHadRejection: $lastRoundHadRejection, recurringKeys: $recurringKeys, fixAttempts: $fixAttempts},
    backoff: {idleStreak: $idleStreak, intervalMinutes: $intervalMinutes, waitSeconds: $waitSeconds},
    errors: [], snapshotPath: $snapshotPath, statePath: $statePath
  }
}
EOF
}

combined=$(jq -n \
  --slurpfile snapfile "$snapshot" --argjson prev "$prev_json" \
  --arg now "$now" --arg statePath "$state_path" --arg snapshotPath "$snapshot_path" \
  '$snapfile[0] as $snap | $prev as $prev | $now as $now | $statePath as $statePath | $snapshotPath as $snapshotPath | '"$(pb_jq_reduce)") \
  || pb_fail invalid_json "reduce-state.sh: reducer failed on $snapshot" '{"source":"reduce"}'

if [ -n "$state_out" ]; then
  pb_atomic_write_json "$state_out" jq -c '.state' <<<"$combined" || pb_fail invalid_json "reduce-state.sh: could not write $state_out" '{"source":"state_out"}'
fi
if [ -n "$result_out" ]; then
  pb_atomic_write_json "$result_out" jq -c '.result' <<<"$combined" || pb_fail invalid_json "reduce-state.sh: could not write $result_out" '{"source":"result_out"}'
else
  jq -c '.result' <<<"$combined"
fi
