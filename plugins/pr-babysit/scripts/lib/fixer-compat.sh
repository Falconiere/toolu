#!/usr/bin/env bash
# Shared Bash plumbing retained for the fixer scripts.
#
# Sourced, never run. Provides common errors, atomic writes, slot locks, and
# state updates for dispatch-fix.sh, route-fix.sh, and fixer-report.sh. The
# babysit controller and GitHub write commands use their Bun bundles.
#
# Portability contract (see docs/toolu/specs/2026-09-19-pr-babysit-helper-design.md):
# bash 3.2 — no mapfile/readarray, no declare -A, no wait -n, no ${var,,}.

# Closed error-code → exit-code map. Adding a code is a schema change.
#   usage 2 · gh_unavailable/jq_required/api_error/invalid_json/head_moved/
#   state_malformed/slot_mismatch 3 · duplicate_reply 4 · resolve_unconfirmed 5
#   locked 75 (EX_TEMPFAIL)
#   Fixer dispatch (route-fix.sh, dispatch-fix.sh): config_invalid/plan_invalid/
#   fixer_running/herdr_unavailable/herdr_error/git_error/worktree_dirty/
#   stale_branch 3
pb_exit_code() {
  case "$1" in
    usage) echo 2 ;;
    duplicate_reply) echo 4 ;;
    resolve_unconfirmed) echo 5 ;;
    locked) echo 75 ;;
    gh_unavailable|jq_required|api_error|invalid_json|head_moved|state_malformed|slot_mismatch) echo 3 ;;
    config_invalid|plan_invalid|fixer_running|herdr_unavailable|herdr_error|git_error|worktree_dirty|stale_branch) echo 3 ;;
    *) echo 3 ;;
  esac
}

# pb_plugin_root -> absolute path of plugins/pr-babysit (this file lives in
# scripts/lib/). Resolved from BASH_SOURCE so an installed copy under any
# host cache path works without configuration.
pb_plugin_root() {
  local here
  here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P) || return 1
  (cd "$here/../.." && pwd -P)
}

# pb_require CMD... -> exit through pb_fail when a dependency is missing.
pb_require() {
  local cmd
  for cmd in "$@"; do
    command -v "$cmd" >/dev/null 2>&1 && continue
    case "$cmd" in
      jq) pb_fail jq_required "jq is required" ;;
      gh) pb_fail gh_unavailable "gh is required" ;;
      *)  pb_fail gh_unavailable "$cmd is required" ;;
    esac
  done
}

# pb_error CODE MESSAGE [EXTRA_JSON] -> one structured error document on stdout.
# EXTRA_JSON (an object) is merged into the error entry.
pb_error() {
  local code="$1" message="$2" extra="${3:-}"
  [ -n "$extra" ] || extra='{}'
  jq -nc --arg code "$code" --arg message "$message" --argjson extra "$extra" \
    '{version:1, errors:[({code:$code, message:$message} + $extra)]}'
}

# pb_fail CODE MESSAGE [EXTRA_JSON] -> emit the error and exit with its code.
pb_fail() {
  pb_error "$@"
  exit "$(pb_exit_code "$1")"
}

# pb_capture VAR CMD [ARGS...] -> run CMD and store its stdout in VAR. When CMD
# fails (typically pb_fail inside the subshell), re-emit its stdout — the
# structured error — and exit with its code, so an error raised under $(...)
# is never swallowed into a variable.
pb_capture() {
  local __pb_var="$1" __pb_out __pb_rc=0
  shift
  __pb_out=$("$@") || __pb_rc=$?
  if [ "$__pb_rc" -ne 0 ]; then
    printf '%s\n' "$__pb_out"
    exit "$__pb_rc"
  fi
  printf -v "$__pb_var" '%s' "$__pb_out"
}

# pb_now -> ISO-8601 UTC timestamp.
pb_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# pb_json_valid FILE -> 0 when FILE parses as JSON.
pb_json_valid() { jq -e . "$1" >/dev/null 2>&1; }

# pb_atomic_write_json TARGET CMD [ARGS...]
# Run CMD with stdout captured into a unique temp file beside TARGET, then
# rename it over TARGET only when CMD exited 0 AND the output parses as JSON.
# Any failure removes the temp file and leaves TARGET byte-identical, so a
# reader never sees a partial document and a crashed writer leaves no residue.
# The in-flight temp file is published so pb_on_exit can remove it when a
# signal lands mid-write: PB_ATOMIC_PREFIX is set BEFORE mktemp runs (a trap
# can fire between mktemp returning and the assignment below, so the exact
# name may never be recorded), and it carries $$ so the sweep only ever
# touches this process's own temps.
PB_ATOMIC_TMP=""
PB_ATOMIC_PREFIX=""
pb_atomic_write_json() {
  local target="$1"; shift
  local dir tmp
  dir=$(dirname "$target")
  mkdir -p "$dir" || return 1
  PB_ATOMIC_PREFIX="$dir/.$(basename "$target").tmp.$$."
  if ! tmp=$(mktemp "${PB_ATOMIC_PREFIX}XXXXXX"); then
    PB_ATOMIC_PREFIX=""; return 1
  fi
  PB_ATOMIC_TMP="$tmp"
  if ! "$@" >"$tmp"; then
    rm -f "$tmp"; PB_ATOMIC_TMP=""; PB_ATOMIC_PREFIX=""; return 1
  fi
  if ! pb_json_valid "$tmp"; then
    rm -f "$tmp"; PB_ATOMIC_TMP=""; PB_ATOMIC_PREFIX=""; return 1
  fi
  if ! mv -f "$tmp" "$target"; then
    rm -f "$tmp"; PB_ATOMIC_TMP=""; PB_ATOMIC_PREFIX=""; return 1
  fi
  PB_ATOMIC_TMP=""; PB_ATOMIC_PREFIX=""
}

# pb_retry_on_rc RETRY_RC ATTEMPTS CMD [ARGS...]
# Run CMD; while it returns exactly RETRY_RC and attempts remain, run it
# again. Returns CMD's last exit code. Used for "collect again once when the
# PR head moved under us": any other failure propagates on the first try.
pb_retry_on_rc() {
  local retry_rc="$1" attempts="$2"; shift 2
  local n=0 rc=0
  while :; do
    n=$((n + 1))
    rc=0
    "$@" || rc=$?
    [ "$rc" -eq "$retry_rc" ] || return "$rc"
    [ "$n" -lt "$attempts" ] || return "$rc"
    echo "pr-babysit: attempt $n of $attempts returned $rc; retrying $1" >&2
  done
}

# pb_mktmpdir -> create the per-run scratch dir (removed by pb_on_exit).
pb_mktmpdir() {
  PB_TMPDIR=$(mktemp -d "${TMPDIR:-/tmp}/pr-babysit.XXXXXX") || return 1
  export PB_TMPDIR
}

# pb_on_exit — the one EXIT handler: release the slot lock (if held) and
# remove the scratch dir. Installed by pb_init; scripts never add a second
# EXIT trap. Runs on signals too (TERM/INT re-raise through EXIT).
pb_on_exit() {
  local rc=$?
  # A second signal must not re-enter `exit` and abort the cleanup midway.
  trap '' TERM INT
  if declare -F pb_lock_release >/dev/null 2>&1; then pb_lock_release; fi
  [ -n "${PB_ATOMIC_TMP:-}" ] && rm -f "$PB_ATOMIC_TMP"
  [ -n "${PB_ATOMIC_PREFIX:-}" ] && rm -f "${PB_ATOMIC_PREFIX}"*
  [ -n "${PB_TMPDIR:-}" ] && [ -d "${PB_TMPDIR:-}" ] && rm -rf "$PB_TMPDIR"
  return "$rc"
}

# pb_init — install the EXIT handler and forward TERM/INT into it.
pb_init() {
  trap pb_on_exit EXIT
  trap 'exit 143' TERM
  trap 'exit 130' INT
}
# One-writer lock per slot state file.
#
# `mkdir` is atomic on every POSIX filesystem, so the
# lock is a directory beside the state file: <state-file>.lock/ holding `pid`
# and `since` (epoch seconds). Atomic rename alone does not stop two
# controllers from losing each other's updates — this does. A lock whose pid is
# dead or older than PB_LOCK_STALE_SECONDS is stale and reclaimed with a stderr
# note. Release is wired through pb_on_exit above so a crash or signal
# never leaves a live lock behind.

PB_LOCK_STALE_SECONDS="${PB_LOCK_STALE_SECONDS:-600}"
PB_LOCK_DIR=""
# The lock dir this process is trying to take. A signal can land between
# `mkdir` succeeding and PB_LOCK_DIR being set; pb_lock_release uses this to
# recognise (and remove) such a half-written lock of its own on exit.
PB_LOCK_WANT=""
PB_LOCK_HOLDER_PID=""
PB_LOCK_HOLDER_SINCE=""

# pb_lock_path STATE_FILE -> the lock directory path.
pb_lock_path() { printf '%s.lock' "$1"; }

# _pb_lock_stale LOCKDIR -> 0 when the holder is dead or too old.
_pb_lock_stale() {
  local lockdir="$1" pid since now
  pid=$(cat "$lockdir/pid" 2>/dev/null || echo "")
  since=$(cat "$lockdir/since" 2>/dev/null || echo "")
  now=$(date +%s)
  # A lock dir with no pid/since is a half-written lock from a crashed taker.
  [ -n "$pid" ] && [ -n "$since" ] || return 0
  case "$pid$since" in *[!0-9]*) return 0 ;; esac
  if ! kill -0 "$pid" 2>/dev/null; then return 0; fi
  [ $((now - since)) -gt "$PB_LOCK_STALE_SECONDS" ] && return 0
  return 1
}

# pb_lock_acquire STATE_FILE -> 0 and PB_LOCK_DIR set; 75 when a live holder
# owns it (PB_LOCK_HOLDER_PID/SINCE describe it). Reclaims a stale lock once.
pb_lock_acquire() {
  local state_file="$1" lockdir attempt
  lockdir=$(pb_lock_path "$state_file")
  mkdir -p "$(dirname "$state_file")" || return 1
  PB_LOCK_WANT="$lockdir"
  for attempt in 1 2; do
    if mkdir "$lockdir" 2>/dev/null; then
      PB_LOCK_DIR="$lockdir"
      printf '%s\n' "$$" >"$lockdir/pid"
      date +%s >"$lockdir/since"
      return 0
    fi
    PB_LOCK_HOLDER_PID=$(cat "$lockdir/pid" 2>/dev/null || echo "")
    PB_LOCK_HOLDER_SINCE=$(cat "$lockdir/since" 2>/dev/null || echo "")
    if [ "$attempt" -eq 1 ] && _pb_lock_stale "$lockdir"; then
      echo "pr-babysit: reclaiming stale lock $lockdir (pid ${PB_LOCK_HOLDER_PID:-?}, since ${PB_LOCK_HOLDER_SINCE:-?})" >&2
      rm -rf "$lockdir"
      continue
    fi
    return 75
  done
  return 75
}

# pb_lock_release — remove the lock only if this process took it.
pb_lock_release() {
  if [ -n "$PB_LOCK_DIR" ]; then
    if [ "$(cat "$PB_LOCK_DIR/pid" 2>/dev/null)" = "$$" ]; then
      rm -rf "$PB_LOCK_DIR"
    fi
  elif [ -n "$PB_LOCK_WANT" ] && [ -d "$PB_LOCK_WANT" ] && [ ! -e "$PB_LOCK_WANT/pid" ]; then
    # mkdir succeeded but a signal arrived before the pid was written: the
    # dir is ours and empty — take it back rather than leave a stale lock.
    rm -rf "$PB_LOCK_WANT"
  fi
  PB_LOCK_DIR=""; PB_LOCK_WANT=""
  return 0
}

# pb_lock_fail STATE_FILE — emit the structured `locked` error and exit 75.
pb_lock_fail() {
  pb_fail locked "slot is held by another controller" \
    "$(jq -nc --arg pid "$PB_LOCK_HOLDER_PID" --arg since "$PB_LOCK_HOLDER_SINCE" \
        '{pid:($pid|tonumber? // null), since:($since|tonumber? // null)}')"
}
# Load and update one slot's state file for the fixer scripts.
#
# dispatch-fix.sh and its helper take the slot lock, validate state, and
# persist fixer results atomically. These functions share that path.

# pb_state_load STATE_FILE — validate (version 2) and export the slot
# identity: PB_STATE_REPO, PB_STATE_NUMBER, PB_STATE_HEAD. Exits through
# pb_fail on a missing or malformed file.
pb_state_load() {
  local state_file="$1"
  [ -f "$state_file" ] || pb_fail state_malformed "state file not found: $state_file (run babysit-tick.js first)" '{"source":"state"}'
  pb_json_valid "$state_file" || pb_fail state_malformed "state file is not valid JSON: $state_file" '{"source":"state"}'
  jq -e '.version == 2 and (.repo | type == "string") and (.number | type == "number")' "$state_file" >/dev/null 2>&1 \
    || pb_fail state_malformed "state file is not a version-2 pr-babysit state: $state_file" "$(jq -c '{source:"state", version:(.version // null)}' "$state_file")"
  PB_STATE_REPO=$(jq -r '.repo' "$state_file")
  PB_STATE_NUMBER=$(jq -r '.number' "$state_file")
  # These are spliced into REST paths: refuse anything but owner/name and an
  # integer, whatever a hand-edited state file says.
  # GitHub owner/name: no leading dot, so `.`/`..` segments cannot form.
  [[ "$PB_STATE_REPO" =~ ^[A-Za-z0-9][A-Za-z0-9_-]*/[A-Za-z0-9_][A-Za-z0-9._-]*$ ]] \
    || pb_fail state_malformed "state file repo is not owner/name: $PB_STATE_REPO" '{"source":"state"}'
  [[ "$PB_STATE_NUMBER" =~ ^[0-9]+$ ]] \
    || pb_fail state_malformed "state file number is not an integer: $PB_STATE_NUMBER" '{"source":"state"}'
  PB_STATE_HEAD=$(jq -r '.pr.headSha // ""' "$state_file")
  export PB_STATE_REPO PB_STATE_NUMBER PB_STATE_HEAD
}

# pb_state_update STATE_FILE FILTER [JQ_ARGS...] — apply a jq filter to the
# state and write it back atomically. The filter must yield the whole state.
pb_state_update() {
  local state_file="$1" filter="$2"; shift 2
  pb_atomic_write_json "$state_file" jq -c "$@" "$filter" "$state_file" \
    || pb_fail state_malformed "could not update $state_file" '{"source":"state"}'
}
