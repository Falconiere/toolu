#!/usr/bin/env bash
# state.sh — load and update one slot's state file for the write-side scripts.
#
# Sourced after common.sh and lock.sh. The write side (reply-thread.sh,
# resolve-thread.sh, record.sh) always: takes the slot lock, validates the
# state, performs its action, and records the outcome atomically. This file
# holds the shared load/update so each script carries only its own action.

# pb_state_load STATE_FILE — validate (version 2) and export the slot
# identity: PB_STATE_REPO, PB_STATE_NUMBER, PB_STATE_HEAD. Exits through
# pb_fail on a missing or malformed file.
pb_state_load() {
  local state_file="$1"
  [ -f "$state_file" ] || pb_fail state_malformed "state file not found: $state_file (run babysit-tick.sh first)" '{"source":"state"}'
  pb_json_valid "$state_file" || pb_fail state_malformed "state file is not valid JSON: $state_file" '{"source":"state"}'
  jq -e '.version == 2 and (.repo | type == "string") and (.number | type == "number")' "$state_file" >/dev/null 2>&1 \
    || pb_fail state_malformed "state file is not a version-2 pr-babysit state: $state_file" "$(jq -c '{source:"state", version:(.version // null)}' "$state_file")"
  PB_STATE_REPO=$(jq -r '.repo' "$state_file")
  PB_STATE_NUMBER=$(jq -r '.number' "$state_file")
  # These are spliced into REST paths: refuse anything but owner/name and an
  # integer, whatever a hand-edited state file says.
  [[ "$PB_STATE_REPO" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]] \
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
