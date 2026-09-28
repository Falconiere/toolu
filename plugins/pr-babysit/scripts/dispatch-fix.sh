#!/usr/bin/env bash
# dispatch-fix.sh — run a routed fix plan (route-fix.sh) on herdr fixer
# agents, one tier group at a time, in the slot's herdr worktree.
#
# Usage: dispatch-fix.sh start   --state-file <p> --plan <route.json> --items <items.json>
#                                --repo-root <dir> --branch <pr-branch> --base <base-branch> [--dry-run]
#        dispatch-fix.sh wait    --state-file <p> [--timeout-seconds N]
#        dispatch-fix.sh cleanup --state-file <p> [--dry-run]
#
#   start    worktree pr-babysit/<slot> (created from, or fast-forwarded to,
#            origin/<pr-branch>) -> fixer record in slot state -> group 1's
#            agent started with its brief. --dry-run prints the commands and
#            the brief and writes nothing.
#   wait     the one fixer command per tick: wait up to --timeout-seconds for
#            the running group; when it settles, record it, exit its agent and
#            start the next group (deferred to the next call when fewer than
#            PB_D_LAUNCH_BUDGET seconds remain). A blocked fixer whose prompt
#            was answered is picked up again. Prints running | done | failed |
#            blocked | none.
#   cleanup  exit a live fixer, clear its record, remove the worktree when it
#            holds no work, delete the branch when origin/<pr-branch> contains
#            it, and remove this slot's fixer brief/report/items files.
#
# The fixer only edits, tests and commits; the controller verifies, pushes,
# replies and resolves. Exit: 0 · 2 usage · 3 structured error · 75 locked.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/lock.sh
. "$PB_SCRIPT_DIR/lib/lock.sh"
# shellcheck source=lib/state.sh
. "$PB_SCRIPT_DIR/lib/state.sh"
# shellcheck source=lib/hosts.sh
. "$PB_SCRIPT_DIR/lib/hosts.sh"
# shellcheck source=lib/fixer.sh
. "$PB_SCRIPT_DIR/lib/fixer.sh"
# shellcheck source=lib/herdr.sh
. "$PB_SCRIPT_DIR/lib/herdr.sh"
# shellcheck source=lib/dispatch.sh
. "$PB_SCRIPT_DIR/lib/dispatch.sh"

sub="${1:-}"; [ $# -gt 0 ] && shift
PB_D_STATE=""; PB_D_PLAN=""; PB_D_DRY=0; PB_D_ROUND=1
plan_file=""; items_file=""; repo_root=""; branch=""; base=""; timeout_s=480
# Settling a group takes seconds; starting the next agent up to ~3 minutes
# when it fails. Start one only with at least this much of the wait left.
PB_D_LAUNCH_BUDGET=60
while [ $# -gt 0 ]; do
  case "$1" in
    --state-file)      PB_D_STATE="${2:-}"; shift 2 ;;
    --plan)            plan_file="${2:-}"; shift 2 ;;
    --items)           items_file="${2:-}"; shift 2 ;;
    --repo-root)       repo_root="${2:-}"; shift 2 ;;
    --branch)          branch="${2:-}"; shift 2 ;;
    --base)            base="${2:-}"; shift 2 ;;
    --timeout-seconds) timeout_s="${2:-}"; shift 2 ;;
    --dry-run)         PB_D_DRY=1; shift ;;
    *) pb_fail usage "dispatch-fix.sh: unknown argument: $1" ;;
  esac
done
case "$sub" in start|wait|cleanup) ;; *) pb_fail usage "dispatch-fix.sh: subcommand must be start, wait or cleanup" ;; esac
[ -n "$PB_D_STATE" ] || pb_fail usage "dispatch-fix.sh: --state-file required"
[[ "$timeout_s" =~ ^[0-9]+$ ]] || pb_fail usage "dispatch-fix.sh: --timeout-seconds must be a whole number"
pb_require jq git
pb_init
pb_state_load "$PB_D_STATE"

# ---------------------------------------------------------------------------
start() {
  [ -n "$plan_file" ] && [ -n "$items_file" ] && [ -n "$repo_root" ] && [ -n "$branch" ] && [ -n "$base" ] \
    || pb_fail usage "dispatch-fix.sh start: --plan, --items, --repo-root, --branch and --base are required"
  pb_json_valid "$plan_file" || pb_fail plan_invalid "plan is missing or not JSON: $plan_file"
  pb_json_valid "$items_file" || pb_fail plan_invalid "items file is missing or not JSON: $items_file"
  PB_D_PLAN=$(jq -c . "$plan_file")
  local problem
  problem=$(jq -r --slurpfile items "$items_file" --slurpfile st "$PB_D_STATE" '
    ([$items[0].items[]?.id]) as $ids | (($st[0].actions.flagged // {}) | keys) as $flagged
    | [.groups[]?.items[]?] as $planned
    | if .dispatch != "herdr" then "the plan dispatches inline; run this round in-session"
      elif (.groups | type) != "array" or (.groups | length) == 0 then "the plan has no groups"
      elif any(.groups[]; .host == null) then "every plan group needs a host"
      elif ($items[0].round // 1 | (type != "number") or . < 1 or (floor != .)) then "round must be a positive integer"
      elif any($planned[]; . as $p | $ids | index([$p]) | not) then "the plan names an item that is not in the items file"
      elif any($planned[]; . as $p | $flagged | index([$p]) != null) then "the plan includes an injection-flagged thread"
      else "" end' "$plan_file")
  [ -z "$problem" ] || pb_fail plan_invalid "$problem"
  pb_d_validate_plan
  case "$(jq -r '.fixer.status // ""' "$PB_D_STATE")" in
    running|blocked)
      pb_fail fixer_running "a fixer is already active for this slot; run dispatch-fix.sh wait" \
        "$(jq -c '{group: .fixer.current, agent: ([.fixer.groups[]? | select(.status == "running" or .status == "launching" or .status == "blocked") | .agent][0] // null)}' "$PB_D_STATE")" ;;
  esac
  PB_D_ROUND=$(jq -r '.round // 1' "$items_file")
  if [ "$PB_D_DRY" -eq 0 ]; then
    pb_herdr_reachable || pb_fail herdr_unavailable "herdr is not reachable (not installed, or its server is not running); run this round inline"
  fi
  local unattended ctx items_copy="${PB_D_STATE%.json}.fixer-items.json"
  pb_d_worktree "$repo_root" "$branch" "$(jq -r '.slot' "$PB_D_STATE")" "$PB_STATE_NUMBER"
  unattended=$(jq -r 'if .unattended == false then "false" else "true" end' <<<"$PB_D_PLAN")
  ctx=$(jq -nc --arg pr "$PB_STATE_REPO#$PB_STATE_NUMBER" --argjson round "$PB_D_ROUND" \
    --argjson groups "$(jq '.groups | length' <<<"$PB_D_PLAN")" --arg wt "$PB_D_WT_PATH" \
    --arg sb "pr-babysit/$(jq -r '.slot' "$PB_D_STATE")" --arg b "$branch" --arg base "$base" \
    '{pr: $pr, round: $round, groups: $groups, worktree: $wt, slotBranch: $sb, branch: $b, base: $base}')
  # Later groups start from `wait`, possibly in another tick: keep what they need.
  [ "$PB_D_DRY" -eq 1 ] || cp "$items_file" "$items_copy"
  pb_d_save '.fixer = {round: $round, status: "running", reason: null, startedAt: $now, current: 1,
                       unattended: ($u == "true"), context: $ctx, itemsFile: $itemsFile,
                       items: [$plan.groups[].items[]],
                       groups: ($plan.groups | map({seq, tier, host, model, effort, items, status: "pending", reason: null}))}' \
    --argjson round "$PB_D_ROUND" --arg now "$(pb_now)" --argjson plan "$PB_D_PLAN" \
    --arg u "$unattended" --argjson ctx "$ctx" --arg itemsFile "$items_copy"
  pb_d_launch 1 "$items_file" "$ctx" "$PB_D_WT_PANE" "$unattended"
  if [ "$PB_D_DRY" -eq 1 ]; then
    jq -nc --argjson c "$PB_D_CMDS" --arg b "$PB_D_BRIEF" '{dryRun: true, commands: $c, brief: $b}'
  else
    pb_d_status
  fi
}

# ---------------------------------------------------------------------------
launch_current() { # the current group was never started, or its start was cut off
  local seq agent
  seq=$(jq -r '.fixer.current' "$PB_D_STATE")
  agent=$(jq -r --argjson s "$seq" '.fixer.groups[] | select(.seq == $s) | .agent // ""' "$PB_D_STATE")
  [ -z "$agent" ] || pb_herdr_agent_stop "$agent" || true
  pb_d_launch "$seq" "$(jq -r '.fixer.itemsFile' "$PB_D_STATE")" "$(jq -c '.fixer.context' "$PB_D_STATE")" \
    "$(jq -r '.herdrWorktree.paneId' "$PB_D_STATE")" "$(jq -r 'if .fixer.unattended == false then "false" else "true" end' "$PB_D_STATE")"
}

settle_group() { # SEQ -> read the settled agent's outcome, exit it, record it
  local seq="$1" g agent pane outcome head=""
  g=$(jq -c --argjson s "$seq" '.fixer.groups[] | select(.seq == $s)' "$PB_D_STATE")
  agent=$(jq -r '.agent' <<<"$g")
  pane=$(herdr agent read "$agent" --source recent-unwrapped --lines 40 2>/dev/null || true)
  outcome=$(pb_fixer_settle "$(jq -r '.report' <<<"$g")" "$pane")
  pb_herdr_agent_stop "$agent" || true
  [ "$outcome" = "done" ] && head=$(git -C "$(jq -r '.herdrWorktree.path' "$PB_D_STATE")" rev-parse HEAD)
  pb_d_settle "$seq" "$outcome" "$head"
}

wait_fixer() {
  local deadline seq groups gstatus agent remaining out st
  if [ "$(jq -r '.fixer // null | type' "$PB_D_STATE")" != object ]; then
    jq -nc '{version: 1, status: "none"}'
    return 0
  fi
  deadline=$(( $(date +%s) + timeout_s ))
  if [ "$(jq -r '.fixer.status' "$PB_D_STATE")" = blocked ]; then
    # Someone answered the prompt (or the agent exited): pick the group up again.
    seq=$(jq -r '.fixer.current' "$PB_D_STATE")
    agent=$(jq -r --argjson s "$seq" '.fixer.groups[] | select(.seq == $s) | .agent' "$PB_D_STATE")
    if [ "$(pb_herdr_agent_status "$agent")" != blocked ]; then
      pb_d_save '.fixer.status = "running" | .fixer.reason = null'
      pb_d_group_set "$seq" '{"status": "running", "reason": null}'
    fi
  fi
  PB_D_ROUND=$(jq -r '.fixer.round' "$PB_D_STATE")
  while [ "$(jq -r '.fixer.status' "$PB_D_STATE")" = running ]; do
    remaining=$(( deadline - $(date +%s) ))
    [ "$remaining" -gt 0 ] || break
    seq=$(jq -r '.fixer.current' "$PB_D_STATE"); groups=$(jq -r '.fixer.groups | length' "$PB_D_STATE")
    gstatus=$(jq -r --argjson s "$seq" '.fixer.groups[] | select(.seq == $s) | .status' "$PB_D_STATE")
    if [ "$gstatus" = pending ] || [ "$gstatus" = launching ]; then
      [ "$remaining" -ge "$PB_D_LAUNCH_BUDGET" ] || break
      launch_current
      continue
    fi
    agent=$(jq -r --argjson s "$seq" '.fixer.groups[] | select(.seq == $s) | .agent' "$PB_D_STATE")
    if out=$(pb_herdr_try agent wait "$agent" --timeout "$((remaining * 1000))"); then
      st=$(jq -r '.agent.agent_status // "unknown"' <<<"$out")
    else
      case "$(jq -r '.error.code' <<<"$out")" in
        timeout) break ;;
        agent_not_found) st=exited ;;
        *) pb_fail herdr_error "herdr agent wait $agent: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$out")" ;;
      esac
    fi
    if [ "$st" = blocked ]; then
      pb_d_save '.fixer.status = "blocked" | .fixer.reason = "agent_blocked"'
      pb_d_group_set "$seq" '{"status": "blocked", "reason": "agent_blocked"}'
      break
    fi
    settle_group "$seq"
    [ "$(jq -r '.fixer.status' "$PB_D_STATE")" = running ] || break
    if [ "$seq" -ge "$groups" ]; then
      pb_d_save '.fixer.status = "done" | .fixer.finishedAt = $now' --arg now "$(pb_now)"
      break
    fi
    # The next group starts at the top of the loop, or in the next call.
    pb_d_save '.fixer.current = $n' --argjson n "$((seq + 1))"
  done
  pb_d_status
}

# ---------------------------------------------------------------------------
cleanup() {
  local wt path ws root pr branch agent removed=false deleted=false note=null out f
  agent=$(jq -r '[.fixer.groups[]? | select(.status == "running" or .status == "launching" or .status == "blocked") | .agent][0] // ""' "$PB_D_STATE")
  if [ -n "$agent" ] && [ "$PB_D_DRY" -eq 0 ]; then pb_herdr_agent_stop "$agent" || true; fi
  # Its agent is gone: nothing is in flight any more.
  pb_d_save '.fixer = null'
  wt=$(jq -c '.herdrWorktree // null' "$PB_D_STATE")
  if [ "$wt" != null ]; then
    path=$(jq -r '.path' <<<"$wt"); ws=$(jq -r '.workspaceId' <<<"$wt"); root=$(jq -r '.repoRoot' <<<"$wt")
    pr=$(jq -r '.prBranch' <<<"$wt"); branch=$(jq -r '.branch' <<<"$wt")
    if [ "$PB_D_DRY" -eq 1 ] || [ -d "$path" ]; then
      pb_d_clean_or_fail "$path"
      if [ "$PB_D_DRY" -eq 0 ]; then
        pb_herdr_reachable || pb_fail herdr_unavailable "herdr is not reachable; cannot remove workspace $ws"
      fi
      # Clean apart from host session artifacts (pb_d_dirt): --force discards only those.
      pb_d_record herdr worktree remove --workspace "$ws" --force
      out=$(pb_d_herdr worktree remove --workspace "$ws" --force) || pb_d_cmd git -C "$root" worktree remove --force "$path" >&2 \
        || pb_fail herdr_error "could not remove the fixer worktree $path: $(jq -r '.error.message // ""' <<<"$out")"
      removed=true
    fi
    pb_d_cmd git -C "$root" worktree prune >&2 || true
    pb_d_cmd git -C "$root" fetch --quiet origin "$pr" >&2 || true
    if [ "$PB_D_DRY" -eq 1 ] || git -C "$root" rev-parse --verify --quiet "refs/heads/$branch" >/dev/null; then
      if [ "$PB_D_DRY" -eq 0 ] && ! git -C "$root" merge-base --is-ancestor "refs/heads/$branch" "refs/remotes/origin/$pr"; then
        note=$(jq -nc --arg b "$branch" --arg p "$pr" '"kept \($b): it holds commits origin/\($p) does not"')
      elif pb_d_cmd git -C "$root" branch --quiet -D "$branch" >&2; then
        deleted=true
      else
        note=$(jq -nc --arg b "$branch" '"kept \($b): git could not delete it (still checked out elsewhere?)"')
      fi
    fi
    pb_d_save '.herdrWorktree = null'
  else
    note='"no herdr worktree recorded"'
  fi
  # This slot's own brief, report and items copies (they quote reviewer text).
  if [ "$PB_D_DRY" -eq 0 ]; then
    for f in "${PB_D_STATE%.json}".fixer-*; do [ -e "$f" ] && rm -f "$f"; done
  fi
  if [ "$PB_D_DRY" -eq 1 ]; then
    jq -nc --argjson c "$PB_D_CMDS" '{dryRun: true, commands: $c}'
  else
    jq -nc --argjson r "$removed" --argjson d "$deleted" --argjson n "$note" \
      '{version: 1, status: "cleaned", worktreeRemoved: $r, branchDeleted: $d, note: $n}'
  fi
}

case "$sub" in
  start) start ;;
  wait) wait_fixer ;;
  cleanup) cleanup ;;
esac
