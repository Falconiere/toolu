#!/usr/bin/env bash
# gh.sh — bounded `gh` transport for the pr-babysit helper.
#
# Sourced after common.sh. Every GitHub call goes through pb_gh so timeouts,
# retries, and error classification live in one place:
#
#   pb_run_with_timeout SECONDS OUT ERR CMD...  run CMD, kill it after SECONDS
#   pb_gh_classify RC ERR_FILE OUT_FILE         -> transient | permanent | ok
#   pb_gh OUT_FILE ARGS...                      gh ARGS with retry; stdout → OUT_FILE
#   pb_gh_json_ok FILE                          -> 0 when FILE is JSON without errors[]
#
# GNU `timeout` is absent on macOS and `gh` has no timeout flag, so the
# watchdog is a background job plus a sleeping killer subshell — bash 3.2 has
# no `wait -n`, so the job's exit code is read from a status file it writes.
#
# Retry policy (spec §Architecture): 3 attempts, 2/4/8 s backoff, for
# timeouts, 5xx, connection errors, HTTP 429, and 403 whose body mentions the
# rate limit. Any other 4xx is permanent and never retried. Observed real
# shapes this classifies (2026-09-19, gh 2.101.0):
#   refused host  → rc 1, stderr `dial tcp 127.0.0.1:1: connect: connection refused`
#   404           → rc 1, stdout `{"message":"Not Found",…,"status":"404"}`, stderr `gh: Not Found (HTTP 404)`
#   GraphQL miss  → rc 1, stdout `{"data":{…null…},"errors":[{"type":"NOT_FOUND",…}]}`

PB_GH_TIMEOUT="${PB_GH_TIMEOUT:-60}"
PB_GH_ATTEMPTS="${PB_GH_ATTEMPTS:-3}"
PB_GH_BACKOFF="${PB_GH_BACKOFF:-2 4 8}"
# Exit code the watchdog reports for a killed command (mirrors GNU timeout).
PB_GH_RC_TIMEOUT=124

# pb_run_with_timeout SECONDS OUT ERR CMD... -> CMD's exit code, or 124 when
# the watchdog killed it. stdout/stderr go to the OUT/ERR files.
pb_run_with_timeout() {
  local secs="$1" out="$2" err="$3"; shift 3
  local rcfile wdfile pid watchdog sleeper rc
  rcfile=$(mktemp "${PB_TMPDIR:-${TMPDIR:-/tmp}}/pb-rc.XXXXXX") || return 1
  wdfile="$rcfile.wd"
  : >"$rcfile"; : >"$wdfile"
  (
    "$@" >"$out" 2>"$err"
    echo $? >"$rcfile"
  ) &
  pid=$!
  # The watchdog detaches from the caller's stdio and publishes its sleeper's
  # pid: killing only the subshell would orphan `sleep`, which then holds any
  # pipe the caller (bats, a `$(...)`) is waiting on until the timeout elapses.
  (
    sleep "$secs" &
    echo $! >"$wdfile"
    wait $!
    kill "$pid" 2>/dev/null
  ) >/dev/null 2>&1 &
  watchdog=$!
  # Every wait/kill below may legitimately fail (job already gone, watchdog
  # killed → wait returns 143); callers run under `set -e`, so none of them
  # may be a bare statement.
  wait "$pid" 2>/dev/null || true
  sleeper=$(cat "$wdfile" 2>/dev/null || true)
  [ -z "$sleeper" ] || kill "$sleeper" 2>/dev/null || true
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  rc=$(cat "$rcfile" 2>/dev/null || true)
  rm -f "$rcfile" "$wdfile"
  if [ -z "$rc" ]; then
    echo "pr-babysit: timed out after ${secs}s: $*" >>"$err"
    return "$PB_GH_RC_TIMEOUT"
  fi
  return "$rc"
}

# pb_gh_classify RC ERR_FILE OUT_FILE -> prints transient | permanent | ok.
pb_gh_classify() {
  local rc="$1" err="$2" out="$3" text status
  [ "$rc" -eq 0 ] && { echo ok; return 0; }
  [ "$rc" -eq "$PB_GH_RC_TIMEOUT" ] && { echo transient; return 0; }
  # `set -e` callers: every pipeline that may match nothing ends in `|| true`.
  text=$( { cat "$err" 2>/dev/null; cat "$out" 2>/dev/null; } || true)
  # HTTP status: gh prints "(HTTP NNN)" on stderr; REST bodies carry "status".
  status=$(printf '%s' "$text" | grep -oE '\(HTTP [0-9]{3}\)' | head -1 | tr -dc '0-9' || true)
  [ -n "$status" ] || status=$(jq -r '.status? // empty' "$out" 2>/dev/null | head -1 || true)
  case "$status" in
    5??|429) echo transient; return 0 ;;
    403)
      if printf '%s' "$text" | grep -qi 'rate limit'; then echo transient; else echo permanent; fi
      return 0 ;;
    4??) echo permanent; return 0 ;;
  esac
  if printf '%s' "$text" | grep -qiE 'connection refused|connection reset|no such host|i/o timeout|TLS handshake|EOF|network is unreachable|temporary failure|timed out'; then
    echo transient; return 0
  fi
  # GraphQL errors[] without an HTTP status (NOT_FOUND etc.) are permanent.
  if jq -e '.errors? | type == "array" and length > 0' "$out" >/dev/null 2>&1; then
    echo permanent; return 0
  fi
  echo permanent
}

# pb_gh OUT_FILE ARGS... -> run `gh ARGS` with timeout + bounded retry.
# 0 on success (OUT_FILE holds stdout). Non-zero after the policy is
# exhausted or on a permanent error; sets PB_GH_ATTEMPTED, PB_GH_LAST_RC,
# PB_GH_LAST_ERR (first stderr line), PB_GH_CLASS for the caller's error.
pb_gh() {
  local out="$1"; shift
  local err attempt rc class delay i
  err="$out.err"
  PB_GH_ATTEMPTED=0; PB_GH_LAST_RC=0; PB_GH_LAST_ERR=""; PB_GH_CLASS=ok
  attempt=0
  while [ "$attempt" -lt "$PB_GH_ATTEMPTS" ]; do
    attempt=$((attempt + 1))
    PB_GH_ATTEMPTED=$attempt
    rc=0
    pb_run_with_timeout "$PB_GH_TIMEOUT" "$out" "$err" gh "$@" || rc=$?
    class=$(pb_gh_classify "$rc" "$err" "$out")
    PB_GH_LAST_RC=$rc; PB_GH_CLASS=$class
    PB_GH_LAST_ERR=$(head -n 1 "$err" 2>/dev/null || true)
    case "$class" in
      ok) rm -f "$err"; return 0 ;;
      permanent) return 1 ;;
    esac
    [ "$attempt" -lt "$PB_GH_ATTEMPTS" ] || break
    i=0; delay=2
    for delay in $PB_GH_BACKOFF; do
      i=$((i + 1))
      [ "$i" -eq "$attempt" ] && break
    done
    echo "pr-babysit: gh $1 attempt $attempt failed ($class: ${PB_GH_LAST_ERR:-rc $rc}); retrying in ${delay}s" >&2
    sleep "$delay"
  done
  return 1
}

# pb_gh_json_ok FILE -> 0 when FILE parses and carries no GraphQL errors[].
pb_gh_json_ok() {
  jq -e 'type as $t | if $t == "object" then ((.errors? // []) | length) == 0 else true end' "$1" >/dev/null 2>&1
}

# pb_gh_fail SOURCE — emit api_error/invalid_json from the last pb_gh state
# and exit 3. SOURCE names the read that failed (pr, threads, comments, …).
pb_gh_fail() {
  local source="$1" code=api_error
  [ "${PB_GH_CLASS:-}" = invalid_json ] && code=invalid_json
  pb_fail "$code" "gh ${source} failed after ${PB_GH_ATTEMPTED:-0} attempt(s): ${PB_GH_LAST_ERR:-rc ${PB_GH_LAST_RC:-?}}" \
    "$(jq -nc --arg source "$source" --argjson attempts "${PB_GH_ATTEMPTED:-0}" \
        --arg class "${PB_GH_CLASS:-}" --arg lastMessage "${PB_GH_LAST_ERR:-}" \
        '{source:$source, attempts:$attempts, class:$class, lastMessage:$lastMessage}')"
}
