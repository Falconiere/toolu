#!/usr/bin/env bash
# dispatch.sh — the steps behind dispatch-fix.sh: record-and-run commands (so
# --dry-run prints exactly what a live run executes), slot-state writes under
# the lock, plan validation, the herdr worktree, launching one fixer group and
# recording how a group settled.
#
# Sourced by dispatch-fix.sh after common/lock/state/hosts/fixer/herdr. Reads
# the globals the entrypoint sets: PB_D_STATE, PB_D_DRY, PB_D_ROUND, PB_D_PLAN.
# Group status: pending -> launching -> running -> done | failed | blocked.

PB_D_CMDS='[]'
PB_D_WT_PATH=""; PB_D_WT_PANE=""; PB_D_BRIEF=""

# pb_d_record ARGV... -> append ARGV to the command list --dry-run prints.
# One --arg per word: jq would parse words like --quiet as its own options.
pb_d_record() {
  local argv='[]' a
  for a in "$@"; do argv=$(jq -c --arg a "$a" '. + [$a]' <<<"$argv"); done
  PB_D_CMDS=$(jq -c --argjson c "$argv" '. + [$c]' <<<"$PB_D_CMDS")
}

# pb_d_cmd ARGV... -> record ARGV and run it unless dry; its status and
# stdout pass through.
pb_d_cmd() {
  pb_d_record "$@"
  [ "$PB_D_DRY" -eq 1 ] && return 0
  "$@"
}

# pb_d_herdr ARGS... -> run `herdr ARGS` through pb_herdr_try unless dry (dry
# prints {}). Returns 1 with herdr's normalized error. Callers capture its
# output with $(...), so they record the argv themselves (pb_d_record).
pb_d_herdr() {
  [ "$PB_D_DRY" -eq 1 ] && { echo '{}'; return 0; }
  pb_herdr_try "$@"
}

# pb_d_save FILTER [JQ_ARGS...] -> apply FILTER to the slot state under the
# slot lock. The lock is held only for the write, never across a wait.
pb_d_save() {
  [ "$PB_D_DRY" -eq 1 ] && return 0
  pb_lock_acquire "$PB_D_STATE" || pb_lock_fail "$PB_D_STATE"
  pb_state_update "$PB_D_STATE" "$@"
  pb_lock_release
}

# pb_d_group_set SEQ JSON_PATCH -> merge JSON_PATCH into group SEQ's record.
pb_d_group_set() {
  pb_d_save '.fixer.groups |= map(if .seq == $s then . + $p else . end)' --argjson s "$1" --argjson p "$2"
}

# pb_d_status -> the dispatcher's status document, from the slot state.
pb_d_status() {
  local wt pr commits='[]'
  wt=$(jq -r '.herdrWorktree.path // ""' "$PB_D_STATE")
  pr=$(jq -r '.herdrWorktree.prBranch // ""' "$PB_D_STATE")
  if [ -n "$wt" ] && [ -d "$wt" ] && [ -n "$pr" ]; then
    # A worktree that is gone or no longer a repository has no commits to report.
    commits=$(git -C "$wt" rev-list --reverse "refs/remotes/origin/$pr..HEAD" 2>/dev/null | jq -Rsc 'split("\n") | map(select(length > 0))') \
      || commits='[]'
  fi
  jq -c --argjson commits "$commits" '
    {version: 1, status: (.fixer.status // "none"), reason: (.fixer.reason // null),
     group: (.fixer.current // null), worktree: (.herdrWorktree.path // null),
     branch: (.herdrWorktree.branch // null), commits: $commits,
     groups: [(.fixer.groups // [])[] | {seq, tier, host, model, effort, agent, status, reason} + (if .error then {error} else {} end)]}' "$PB_D_STATE"
}

# pb_d_validate_plan -> exit config_invalid before any side effect when a
# group names a host other than claude/codex/cursor or carries a model/effort
# the pane shell would mangle (pb_agent_args checks it).
pb_d_validate_plan() {
  local g host argv=""
  while IFS= read -r g; do
    host=$(jq -r '.host' <<<"$g")
    [ "$(pb_host_kind_try "$host" || true)" = "$host" ] \
      || pb_fail config_invalid "plan group $(jq -r '.seq' <<<"$g") names host '$host'; use claude, codex or cursor"
    pb_capture argv pb_agent_args "$host" pb-000000-r1g1 "$(jq -r '.model // ""' <<<"$g")" "$(jq -r '.effort // ""' <<<"$g")" true auto
  done < <(jq -c '.groups[]' <<<"$PB_D_PLAN")
}

# pb_d_dirt DIR -> `git status --porcelain` lines that are real uncommitted
# work. Untracked files under .claude/, .codex/ or .cursor/ are the agent
# hosts' own session artifacts (a fixer's SessionStart hooks write
# .claude/settings.local.json and .claude/tmp/ there; push-review state lives
# there too) and are not work.
pb_d_dirt() {
  git -C "$1" status --porcelain --untracked-files=all | grep -vE '^\?\? \.(claude|codex|cursor)/' || true
}

# pb_d_clean_or_fail DIR -> exit worktree_dirty when DIR has uncommitted work.
pb_d_clean_or_fail() {
  local dirt
  [ "$PB_D_DRY" -eq 1 ] && return 0
  dirt=$(pb_d_dirt "$1")
  [ -z "$dirt" ] && return 0
  pb_fail worktree_dirty "fixer worktree has uncommitted changes: $1" \
    "$(jq -nc --arg p "$1" --arg d "$dirt" '{path: $p, changes: ($d | split("\n"))}')"
}

# pb_d_drop_branch ROOT BRANCH PR -> delete a leftover local BRANCH that
# origin/PR already contains; exit stale_branch when it holds other commits or
# git refuses (it is still checked out somewhere).
pb_d_drop_branch() {
  local root="$1" branch="$2" pr="$3"
  [ "$PB_D_DRY" -eq 1 ] && return 0
  git -C "$root" rev-parse --verify --quiet "refs/heads/$branch" >/dev/null || return 0
  git -C "$root" merge-base --is-ancestor "refs/heads/$branch" "refs/remotes/origin/$pr" \
    || pb_fail stale_branch "local $branch holds commits origin/$pr does not; inspect it before babysit reuses the name" \
         "$(jq -nc --arg b "$branch" '{branch:$b}')"
  pb_d_cmd git -C "$root" branch --quiet -D "$branch" >&2 \
    || pb_fail stale_branch "git could not delete $branch (is it checked out in another worktree?)" \
         "$(jq -nc --arg b "$branch" '{branch:$b}')"
}

# pb_d_worktree REPO_ROOT PR_BRANCH SLOT NUMBER -> ensure the slot's herdr
# worktree on pr-babysit/<slot>, fast-forwarded to origin/<PR_BRANCH>, and
# record it in state. Sets PB_D_WT_PATH and PB_D_WT_PANE (never call it under
# $(...): the recorded commands and any pb_fail must reach this shell).
pb_d_worktree() {
  local root="$1" pr="$2" number="$4" wt path ws pane out branch="pr-babysit/$3"
  wt=$(jq -c '.herdrWorktree // null' "$PB_D_STATE")
  path=$(jq -r '.path // ""' <<<"$wt")
  if [ -n "$path" ] && { [ "$PB_D_DRY" -eq 1 ] || [ -d "$path" ]; }; then
    pb_d_clean_or_fail "$path"
    pb_d_cmd git -C "$path" fetch --quiet origin "$pr" >&2 || pb_fail git_error "git fetch origin $pr failed in $path"
    pb_d_cmd git -C "$path" merge --quiet --ff-only "refs/remotes/origin/$pr" >&2 \
      || pb_fail stale_branch "$branch cannot fast-forward to origin/$pr (the PR branch was rewritten); run dispatch-fix.sh cleanup, then start again" \
           "$(jq -nc --arg b "$branch" '{branch:$b}')"
    ws=$(jq -r '.workspaceId // ""' <<<"$wt"); pane=$(jq -r '.paneId // ""' <<<"$wt")
    if [ "$PB_D_DRY" -eq 0 ] && ! pb_herdr_try pane list --workspace "$ws" | jq -e --arg p "$pane" '(.panes // []) | any(.pane_id == $p)' >/dev/null 2>&1; then
      # The worktree survived but its herdr workspace was closed: reopen it.
      pb_d_record herdr worktree open --cwd "$root" --path "$path" --label "pb-$number" --no-focus
      out=$(pb_d_herdr worktree open --cwd "$root" --path "$path" --label "pb-$number" --no-focus) \
        || pb_fail herdr_error "herdr worktree open: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$out")"
      ws=$(jq -r '.workspace.workspace_id' <<<"$out"); pane=$(jq -r '.root_pane.pane_id' <<<"$out")
    fi
  else
    pb_d_cmd git -C "$root" fetch --quiet origin "$pr" >&2 || pb_fail git_error "git fetch origin $pr failed in $root"
    # A fixer worktree deleted by hand leaves git metadata that would block
    # the create; prune drops only entries whose directory is gone.
    pb_d_cmd git -C "$root" worktree prune >&2 || pb_fail git_error "git worktree prune failed in $root"
    pb_d_drop_branch "$root" "$branch" "$pr"
    pb_d_record herdr worktree create --cwd "$root" --branch "$branch" --base "origin/$pr" --label "pb-$number" --no-focus
    out=$(pb_d_herdr worktree create --cwd "$root" --branch "$branch" --base "origin/$pr" --label "pb-$number" --no-focus) \
      || pb_fail herdr_error "herdr worktree create: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$out")"
    if [ "$PB_D_DRY" -eq 1 ]; then
      path="<herdr worktree path>"; ws="<workspace>"; pane="<root pane>"
    else
      path=$(jq -r '.worktree.path' <<<"$out"); ws=$(jq -r '.workspace.workspace_id' <<<"$out"); pane=$(jq -r '.root_pane.pane_id' <<<"$out")
    fi
  fi
  pb_d_save '.herdrWorktree = {path: $p, workspaceId: $w, paneId: $pn, branch: $b, prBranch: $pr, repoRoot: $r, base: ("origin/" + $pr)}' \
    --arg p "$path" --arg w "$ws" --arg pn "$pane" --arg b "$branch" --arg pr "$pr" --arg r "$root"
  PB_D_WT_PATH="$path"; PB_D_WT_PANE="$pane"
  export PB_D_WT_PATH PB_D_WT_PANE
}

# pb_d_launch SEQ ITEMS_FILE CONTEXT_JSON PANE UNATTENDED -> start group SEQ's
# agent in PANE and hand it its brief. The group is `launching` until the
# prompt lands, then `running`; a failed start marks the group and the fixer
# failed (agent_start_failed, herdr's message in `error`) instead of leaving a
# phantom record. Sets PB_D_BRIEF on a dry run. Never call it under $(...).
pb_d_launch() {
  local seq="$1" items="$2" ctx="$3" pane="$4" unattended="$5" g host model effort agent brief report args a err=""
  local -a argv=()
  if [ "$PB_D_DRY" -eq 1 ]; then
    g=$(jq -c --argjson s "$seq" '.groups[] | select(.seq == $s)' <<<"$PB_D_PLAN")
  else
    g=$(jq -c --argjson s "$seq" '(.fixer.groups // [])[] | select(.seq == $s)' "$PB_D_STATE")
  fi
  host=$(jq -r '.host' <<<"$g"); model=$(jq -r '.model // ""' <<<"$g"); effort=$(jq -r '.effort // ""' <<<"$g")
  agent=$(pb_fixer_agent_name "$(jq -r '.slot' "$PB_D_STATE")" "$PB_D_ROUND" "$seq")
  brief=$(pb_fixer_brief_path "$PB_D_STATE" "$PB_D_ROUND" "$seq")
  report=$(pb_fixer_report_path "$PB_D_STATE" "$PB_D_ROUND" "$seq")
  ctx=$(jq -c --arg d "bash '$(pb_plugin_root)/scripts/fixer-report.sh' '$report' done --note \"<one-line summary>\"" \
              --arg f "bash '$(pb_plugin_root)/scripts/fixer-report.sh' '$report' failed --note \"<the reason>\"" \
              '. + {reportDone: $d, reportFailed: $f}' <<<"$ctx")
  pb_capture args pb_agent_args "$host" "$agent" "$model" "$effort" "$unattended" auto
  while IFS= read -r a; do argv+=("$a"); done <<<"$args"
  if [ "$PB_D_DRY" -eq 1 ]; then
    PB_D_BRIEF=$(pb_fixer_render_brief "$(pb_plugin_root)/skills/babysit/references/fixer-brief.md" "$items" "$g" "$ctx")
    export PB_D_BRIEF
  else
    rm -f "$report"
    pb_fixer_render_brief "$(pb_plugin_root)/skills/babysit/references/fixer-brief.md" "$items" "$g" "$ctx" >"$brief"
  fi
  pb_d_save '.fixer.current = $s' --argjson s "$seq"
  pb_d_group_set "$seq" "$(jq -nc --arg a "$agent" --arg b "$brief" --arg r "$report" --arg now "$(pb_now)" \
    '{agent: $a, brief: $b, report: $r, status: "launching", reason: null, startedAt: $now}')"
  pb_d_record herdr agent start "$agent" --kind "$host" --pane "$pane" --timeout 90000 -- "${argv[@]}"
  pb_d_record herdr agent prompt "$agent" "You are a pr-babysit fixer. Read $brief and follow it exactly." \
    --wait --until working --until blocked --timeout 60000
  # A Claude start that returns agent_not_ready is retried past its trust
  # prompt only when the pane shows exactly the standard prompt for this
  # worktree: pb_herdr_accept_claude_trust reads the screen first and returns
  # at once, without sending a key or waiting, on any other screen. A crashed
  # start or any other dialog therefore fails here with herdr's own message.
  if [ "$PB_D_DRY" -eq 0 ] && ! command -v "$(pb_host_cli "$host")" >/dev/null 2>&1; then
    err="$(pb_host_cli "$host") is not on PATH"
  elif ! err=$(pb_d_herdr agent start "$agent" --kind "$host" --pane "$pane" --timeout 90000 -- "${argv[@]}") \
       && ! { [ "$host" = claude ] && [ "$(jq -r '.error.code' <<<"$err")" = agent_not_ready ] \
              && pb_herdr_accept_claude_trust "$agent" "$(jq -r '.herdrWorktree.path' "$PB_D_STATE")"; }; then
    err="herdr agent start: $(jq -r '"\(.error.code): \(.error.message)" + (if .error.code == "agent_not_ready" then " (the blocked screen is not the standard trust prompt for this worktree; left unanswered)" else "" end)' <<<"$err")"
  elif ! err=$(pb_d_herdr agent prompt "$agent" "You are a pr-babysit fixer. Read $brief and follow it exactly." \
                 --wait --until working --until blocked --timeout 60000); then
    err="herdr agent prompt: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$err")"
  else
    err=""
  fi
  if [ -n "$err" ]; then
    [ "$PB_D_DRY" -eq 1 ] || pb_herdr_agent_stop "$agent" || true
    pb_d_settle "$seq" agent_start_failed "" "$err"
    return 0
  fi
  pb_d_group_set "$seq" '{"status": "running"}'
}

# pb_d_settle SEQ OUTCOME [HEAD] [ERROR] -> record how group SEQ ended.
#   done              the group is done at worktree HEAD
#   host_limited      the host cools down for 60 min; the fixer fails
#   reported_failed | no_report | agent_start_failed | worktree_lost
#                     the fixer fails (ERROR, when given, is kept on the group)
# Pure state transition (no herdr call), so it is tested on real state files.
pb_d_settle() {
  local seq="$1" outcome="$2" head="${3:-}" err="${4:-}" now
  now=$(pb_now)
  case "$outcome" in
    done)
      pb_d_group_set "$seq" "$(jq -nc --arg now "$now" --arg h "$head" '{status: "done", reason: null, finishedAt: $now, head: $h}')" ;;
    host_limited)
      pb_d_save '.hostCooldowns[$host] = {until: (now + 3600 | todate), reason: "host_limited"}
                 | .fixer.status = "failed" | .fixer.reason = "host_limited"
                 | .fixer.groups |= map(if .seq == $s then . + {status: "failed", reason: "host_limited", finishedAt: $now} else . end)' \
        --arg host "$(jq -r --argjson s "$seq" '.fixer.groups[] | select(.seq == $s) | .host' "$PB_D_STATE")" \
        --argjson s "$seq" --arg now "$now" ;;
    *)
      pb_d_save '.fixer.status = "failed" | .fixer.reason = $r
                 | .fixer.groups |= map(if .seq == $s then . + {status: "failed", reason: $r, finishedAt: $now}
                                          + (if $e == "" then {} else {error: $e} end) else . end)' \
        --arg r "$outcome" --arg e "$err" --argjson s "$seq" --arg now "$now" ;;
  esac
}

# pb_d_settle_group SEQ -> read the settled agent's report and last screen,
# exit the agent, and record the outcome (pb_d_settle). A group that reported
# done in a worktree git can no longer read is worktree_lost, not a crash.
pb_d_settle_group() {
  local seq="$1" g agent pane outcome head="" wt read_err=""
  g=$(jq -c --argjson s "$seq" '.fixer.groups[] | select(.seq == $s)' "$PB_D_STATE")
  agent=$(jq -r '.agent' <<<"$g")
  # `agent read` prints the pane as plain text, not JSON, so it is called
  # directly (pb_herdr_try would discard it). A failed read is kept on the
  # group, so a lost screen is never mistaken for a quiet no_report.
  if ! pane=$(herdr agent read "$agent" --source recent-unwrapped --lines 40 2>&1); then
    read_err="could not read the fixer's last screen: $(printf '%s' "$pane" | head -c 160 | tr '\n' ' ' | sed 's/[[:space:]]*$//')"
    pane=""
  fi
  outcome=$(pb_fixer_settle "$(jq -r '.report' <<<"$g")" "$pane")
  pb_herdr_agent_stop "$agent" || true
  if [ "$outcome" = "done" ]; then
    wt=$(jq -r '.herdrWorktree.path // ""' "$PB_D_STATE")
    if ! head=$(git -C "$wt" rev-parse HEAD 2>/dev/null); then
      pb_d_settle "$seq" worktree_lost "" "the fixer reported done, but its worktree is not readable: $wt"
      return 0
    fi
  fi
  pb_d_settle "$seq" "$outcome" "$head" "$([ "$outcome" = no_report ] && printf '%s' "$read_err")"
}
