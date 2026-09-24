#!/usr/bin/env bash
# Tear down one sub-issue after its PR merged: exit the agent, remove the herdr
# worktree workspace, delete the local branch, mark the record merged.
# Usage: finish_issue.sh <state-dir> <key> [--abandon]
# Refuses unless the PR is MERGED (or --abandon is given, which keeps the branch).
set -euo pipefail

state=${1:?state dir}; key=${2:?issue key}; mode=${3:-}
rec="$state/issues/$key.json"
[[ -s $rec ]] || { echo "no record $rec" >&2; exit 1; }

field() { jq -r --arg f "$1" '.[$f] // empty' "$rec"; }
repo=$(field repo) branch=$(field branch) checkout=$(field checkout) ws=$(field workspace_id) agent=$(field agent)
pr=$(jq -r '.pr // empty' "$state/status/$key.json" 2>/dev/null || true)

if [[ $mode != --abandon ]]; then
  [[ -n $pr ]] || { echo "no PR recorded for $key; use --abandon to tear down anyway" >&2; exit 1; }
  pr_state=$(gh pr view "$pr" -R "$repo" --json state -q .state)
  [[ $pr_state == MERGED ]] || { echo "PR $repo#$pr is $pr_state, not MERGED" >&2; exit 1; }
fi

# 1. Exit the agent (babysit's cron is session-scoped and ends with it).
if [[ -n $agent ]] && herdr agent get "$agent" >/dev/null 2>&1; then
  herdr agent send-keys "$agent" esc >/dev/null 2>&1 || true
  herdr agent prompt "$agent" "/exit" >/dev/null 2>&1 || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do herdr agent get "$agent" >/dev/null 2>&1 || break; sleep 1; done
fi

# 2. Remove the worktree workspace. After a merge, leftovers are disposable; record them first.
wt_path=$(field worktree)
dirty=""
[[ -d $wt_path ]] && dirty=$(git -C "$wt_path" status --porcelain 2>/dev/null | head -20 || true)
removed=false
if [[ -n $ws ]]; then
  if [[ $mode == --abandon ]]; then
    herdr worktree remove --workspace "$ws" >/dev/null && removed=true
  else
    herdr worktree remove --workspace "$ws" --force >/dev/null && removed=true
  fi
else
  echo "WARN: no workspace_id on record $key; skipping worktree remove" >&2
fi

# 3. Delete the local branch only when merged (squash merges need -D).
branch_deleted=false
if [[ $mode != --abandon && -n $checkout && -n $branch ]]; then
  git -C "$checkout" branch -D "$branch" >/dev/null 2>&1 && branch_deleted=true
fi

stage=merged; [[ $mode == --abandon ]] && stage=abandoned
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
jq --arg stage "$stage" --arg now "$now" --arg dirty "$dirty" --argjson removed "$removed" \
   --argjson bd "$branch_deleted" \
   '.stage = $stage | .finished_at = $now | .worktree_removed = $removed | .branch_deleted = $bd
    | .leftover_files = ($dirty | split("\n") | map(select(length > 0)))' "$rec" >"$rec.tmp"
mv "$rec.tmp" "$rec"
jq -c '{key, stage, worktree_removed, branch_deleted, leftover_files}' "$rec"
