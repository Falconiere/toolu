#!/usr/bin/env bash
# lock.sh — one-writer lock per slot state file.
#
# Sourced after common.sh. `mkdir` is atomic on every POSIX filesystem, so the
# lock is a directory beside the state file: <state-file>.lock/ holding `pid`
# and `since` (epoch seconds). Atomic rename alone does not stop two
# controllers from losing each other's updates — this does. A lock whose pid is
# dead or older than PB_LOCK_STALE_SECONDS is stale and reclaimed with a stderr
# note. Release is wired through pb_on_exit (common.sh) so a crash or signal
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
