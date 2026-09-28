#!/usr/bin/env bash
# herdr.sh — the herdr CLI surface the fixer dispatcher uses.
#
# Sourced after common.sh. herdr prints one JSON document per call:
# {"result":{...}} on success, {"error":{"code","message"}} on failure. These
# helpers unwrap `.result` (epic-orchestrator common.ts herdr() does the same)
# and normalize failures, so callers branch on herdr's own error codes
# (`timeout`, `agent_not_found`, …) instead of parsing text.

# How long pr-babysit waits for a fixer agent to settle after a trust prompt
# or an exit request, in seconds (polled once a second).
PB_HERDR_WAIT_SECONDS=30

# pb_herdr_try ARGS... -> 0 and the `.result` object on stdout; or 1 and a
# normalized {"error":{"code","message"}} on stdout (non-JSON output becomes
# code `invalid_output`).
pb_herdr_try() {
  local out rc=0
  out=$(herdr "$@" 2>&1) || rc=$?
  if ! jq -e 'type == "object"' >/dev/null 2>&1 <<<"$out"; then
    jq -nc --arg m "$(printf '%s' "$out" | head -c 200)" --argjson rc "$rc" \
      '{error: {code: "invalid_output", message: ("exit \($rc): " + $m)}}'
    return 1
  fi
  if jq -e 'has("error")' >/dev/null <<<"$out"; then
    jq -c '{error: {code: (.error.code // "unknown"), message: (.error.message // "")}}' <<<"$out"
    return 1
  fi
  jq -c '.result // .' <<<"$out"
}

# pb_herdr ARGS... -> the `.result` object, or exit herdr_error naming the call.
pb_herdr() {
  local out
  if out=$(pb_herdr_try "$@"); then
    printf '%s\n' "$out"
    return 0
  fi
  pb_fail herdr_error "herdr $1 $2: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$out")" \
    "$(jq -c '{herdrCode: .error.code}' <<<"$out")"
}

# pb_herdr_reachable -> 0 when the herdr CLI exists and its server answers.
pb_herdr_reachable() {
  command -v herdr >/dev/null 2>&1 || return 1
  pb_herdr_try workspace list >/dev/null
}

# pb_herdr_is_claude_trust_prompt PANE WORKTREE -> 0 when PANE's last screen
# is exactly Claude Code's standard first-run workspace-trust prompt for
# WORKTREE: "Accessing workspace:", the worktree path on its own line, and the
# two options "No, exit" / "Yes, I trust this folder". The variant that also
# asks to accept repository-declared permissions, hooks or MCP servers is
# refused. Its markers are matched on the screen's text with the path line
# removed (a repository named "*hooks*" is fine) and line breaks joined
# (Claude's UI hard-wraps text to the pane width). Pure: reads only its
# arguments.
pb_herdr_is_claude_trust_prompt() {
  local screen prose
  screen=$(printf '%s\n' "$1" | sed 's/[[:space:]]*$//' | awk '/Accessing workspace:/{buf=""} {buf = buf $0 "\n"} END{printf "%s", buf}')
  grep -Fq 'Accessing workspace:' <<<"$screen" || return 1
  grep -Fxq " $2" <<<"$screen" || grep -Fxq "$2" <<<"$screen" || return 1
  grep -Fq 'Yes, I trust this folder' <<<"$screen" || return 1
  grep -Fq 'No, exit' <<<"$screen" || return 1
  prose=$(grep -Fxv -e " $2" -e "$2" <<<"$screen" | tr -s '[:space:]' ' ')
  if grep -Eqi 'only proceed if you trust|trust this configuration|without these permissions|pre-approves|this folder adds|headershelper|mcp server|\bhooks?\b' <<<"$prose"; then
    return 1
  fi
  return 0
}

# pb_herdr_accept_claude_trust NAME WORKTREE -> 0 after answering the standard
# trust prompt (pb_herdr_is_claude_trust_prompt) for exactly WORKTREE — the
# herdr worktree babysit created from the user's own PR branch — and seeing
# the agent become idle. Any other blocked screen returns 1 untouched: the
# same rule as epic-orchestrator's recovery guide.
pb_herdr_accept_claude_trust() {
  local name="$1" worktree="$2" pane waited=0 st
  pane=$(herdr agent read "$name" --source recent-unwrapped --lines 40 2>/dev/null) || return 1
  pb_herdr_is_claude_trust_prompt "$pane" "$worktree" || return 1
  pb_herdr_try agent send-keys "$name" down >/dev/null || return 1
  pb_herdr_try agent send-keys "$name" enter >/dev/null || return 1
  while [ "$waited" -lt "$PB_HERDR_WAIT_SECONDS" ]; do
    st=$(pb_herdr_try agent get "$name" | jq -r '.agent.agent_status // ""') || st=""
    [ "$st" = idle ] && return 0
    sleep 1
    waited=$((waited + 1))
  done
  echo "pr-babysit: $name was still not idle ${PB_HERDR_WAIT_SECONDS}s after the trust prompt" >&2
  return 1
}

# pb_herdr_agent_status NAME -> the live agent's status (idle, working,
# blocked, done, unknown), or `gone` when herdr lists no such agent.
pb_herdr_agent_status() {
  local out
  if out=$(pb_herdr_try agent get "$1"); then
    jq -r '.agent.agent_status // "unknown"' <<<"$out"
  elif [ "$(jq -r '.error.code' <<<"$out")" = agent_not_found ]; then
    echo gone
  else
    pb_fail herdr_error "herdr agent get $1: $(jq -r '"\(.error.code): \(.error.message)"' <<<"$out")"
  fi
}

# pb_herdr_agent_live NAME -> 0 when herdr lists a live agent with that name.
pb_herdr_agent_live() {
  local out
  out=$(pb_herdr_try agent list) || return 1
  jq -e --arg n "$1" '(.agents // []) | any(.name == $n)' >/dev/null <<<"$out"
}

# pb_herdr_agent_stop NAME -> ask a live agent to exit and wait (<= 30 s) until
# herdr no longer lists it. Local work stays in the worktree. Returns 1 when
# it is still alive after the wait.
pb_herdr_agent_stop() {
  local name="$1" waited=0
  pb_herdr_agent_live "$name" || return 0
  pb_herdr_try agent send-keys "$name" esc >/dev/null || true
  pb_herdr_try agent prompt "$name" /exit >/dev/null || true
  while [ "$waited" -lt "$PB_HERDR_WAIT_SECONDS" ]; do
    pb_herdr_agent_live "$name" || return 0
    sleep 1
    waited=$((waited + 1))
  done
  echo "pr-babysit: fixer agent $name did not exit after ${PB_HERDR_WAIT_SECONDS}s" >&2
  return 1
}
