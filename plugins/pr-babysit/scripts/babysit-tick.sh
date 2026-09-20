#!/usr/bin/env bash
# babysit-tick.sh — the one command a babysit tick runs, on either host.
#
#   lock the slot → (validate prior state) → collect → reduce → persist → print
#
# Usage: babysit-tick.sh --repo <owner/repo> --pr <n> --state-file <path>
#                        [--snapshot-out <path>] [--snapshot-in <path>]
#                        [--page-size N] [--timeout SECONDS] [--now <iso8601>]
#   --state-file    the host's slot path, passed verbatim by the workflow
#                   (/tmp/pr-babysit-<slot>.json on Claude,
#                    <repo>/.codex/tmp/pr-babysit/<slot>.json on Codex)
#   --snapshot-out  full snapshot location (default: beside the state file)
#   --snapshot-in   skip collection and reduce a captured snapshot (tests,
#                   replay); its repo/number must match the arguments
#   --now           timestamp override for tests (default: UTC now)
#
# stdout: exactly one JSON document — the result (exit 0) or a structured
# error (non-zero). stderr: diagnostics only.
# Exit: 0 result · 2 usage · 3 structured error, prior state preserved
#       (pr.lastError updated when a prior state exists) · 75 slot locked.
# Nothing here reads a session-specific environment variable for the repo,
# PR, home directory or plugin root: the root comes from this file's path.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/lock.sh
. "$PB_SCRIPT_DIR/lib/lock.sh"

repo=""; number=""; state_file=""; snapshot_out=""; snapshot_in=""; page_size=""; timeout=""; now=""
while [ $# -gt 0 ]; do
  case "$1" in
    --repo)         repo="${2:-}"; shift 2 ;;
    --pr)           number="${2:-}"; shift 2 ;;
    --state-file)   state_file="${2:-}"; shift 2 ;;
    --snapshot-out) snapshot_out="${2:-}"; shift 2 ;;
    --snapshot-in)  snapshot_in="${2:-}"; shift 2 ;;
    --page-size)    page_size="${2:-}"; shift 2 ;;
    --timeout)      timeout="${2:-}"; shift 2 ;;
    --now)          now="${2:-}"; shift 2 ;;
    *) pb_fail usage "babysit-tick.sh: unknown argument: $1" ;;
  esac
done
[[ "$repo" =~ ^[^/[:space:]]+/[^/[:space:]]+$ ]] || pb_fail usage "babysit-tick.sh: --repo <owner/repo> required"
[[ "$number" =~ ^[0-9]+$ ]] || pb_fail usage "babysit-tick.sh: --pr <n> required"
[ -n "$state_file" ] || pb_fail usage "babysit-tick.sh: --state-file <path> required"
[ -z "$page_size" ] || [[ "$page_size" =~ ^[0-9]+$ ]] || pb_fail usage "babysit-tick.sh: --page-size must be an integer"
[ -z "$timeout" ] || [[ "$timeout" =~ ^[0-9]+$ ]] || pb_fail usage "babysit-tick.sh: --timeout must be an integer"
[ -n "$now" ] || now=$(pb_now)
[[ "$now" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || pb_fail usage "babysit-tick.sh: --now must be YYYY-MM-DDTHH:MM:SSZ"
[ -n "$snapshot_out" ] || snapshot_out="${state_file%.json}.snapshot.json"

pb_require jq
[ -n "$snapshot_in" ] || pb_require gh
pb_init
pb_mktmpdir
mkdir -p "$(dirname "$state_file")" || pb_fail usage "babysit-tick.sh: cannot create $(dirname "$state_file")"
pb_lock_acquire "$state_file" || pb_lock_fail "$state_file"

# --- prior state: validate before touching the network ----------------------
# reduce-state.sh repeats these checks; running them first means a malformed
# or foreign state file fails in milliseconds and never costs API calls.
have_state=0
if [ -e "$state_file" ]; then
  have_state=1
  pb_json_valid "$state_file" || pb_fail state_malformed "babysit-tick.sh: state file is not valid JSON: $state_file" '{"source":"state"}'
  jq -e '.version == 2' "$state_file" >/dev/null 2>&1 \
    || pb_fail state_malformed "babysit-tick.sh: state file is not version 2: $state_file" "$(jq -c '{source:"state", version:(.version // null)}' "$state_file")"
  jq -e --arg repo "$repo" --argjson number "$number" '.repo == $repo and .number == $number' "$state_file" >/dev/null 2>&1 \
    || pb_fail slot_mismatch "babysit-tick.sh: state file belongs to $(jq -r '"\(.repo)#\(.number)"' "$state_file"), asked for $repo#$number" \
         "$(jq -c --arg repo "$repo" --argjson number "$number" '{source:"state", state:{repo, number}, requested:{repo:$repo, number:$number}}' "$state_file")"
fi

# _record_error DOC — on a collection failure, keep the prior state but stamp
# pr.lastError so the next reader sees what happened and when.
_record_error() {
  local doc="$1"
  [ "$have_state" -eq 1 ] || return 0
  pb_atomic_write_json "$state_file" jq -c --argjson e "$(jq -c '.errors[0]' <<<"$doc")" --arg now "$now" \
    '.pr.lastError = {code: $e.code, message: $e.message, at: $now}' "$state_file" \
    || echo "babysit-tick.sh: could not record lastError in $state_file" >&2
}

# --- snapshot: collect, or take the one handed in ----------------------------
snap="$PB_TMPDIR/snapshot.json"
if [ -n "$snapshot_in" ]; then
  [ -f "$snapshot_in" ] && pb_json_valid "$snapshot_in" || pb_fail invalid_json "babysit-tick.sh: --snapshot-in is not a JSON file: $snapshot_in" '{"source":"snapshot"}'
  jq -e --arg repo "$repo" --argjson number "$number" '.repo == $repo and .number == $number' "$snapshot_in" >/dev/null 2>&1 \
    || pb_fail slot_mismatch "babysit-tick.sh: --snapshot-in is for $(jq -r '"\(.repo)#\(.number)"' "$snapshot_in"), asked for $repo#$number" '{"source":"snapshot"}'
  cp "$snapshot_in" "$snap"
else
  collect_args=(--repo "$repo" --pr "$number" --out "$snap")
  [ -z "$page_size" ] || collect_args+=(--page-size "$page_size")
  [ -z "$timeout" ] || collect_args+=(--timeout "$timeout")
  rc=0
  bash "$PB_SCRIPT_DIR/collect-pr.sh" "${collect_args[@]}" >"$PB_TMPDIR/collect.out" || rc=$?
  if [ "$rc" -ne 0 ]; then
    doc=$(grep '^{' "$PB_TMPDIR/collect.out" | tail -n 1 || true)
    [ -n "$doc" ] || doc=$(pb_error api_error "babysit-tick.sh: collect-pr.sh exited $rc without a structured error" '{"source":"collect"}')
    _record_error "$doc"
    printf '%s\n' "$doc"
    exit "$rc"
  fi
fi

# --- reduce -------------------------------------------------------------------
rc=0
bash "$PB_SCRIPT_DIR/reduce-state.sh" --snapshot "$snap" --state "$state_file" --now "$now" \
  --state-out "$PB_TMPDIR/state.next.json" --result-out "$PB_TMPDIR/result.json" \
  --state-path "$state_file" --snapshot-path "$snapshot_out" >"$PB_TMPDIR/reduce.out" || rc=$?
if [ "$rc" -ne 0 ]; then
  doc=$(grep '^{' "$PB_TMPDIR/reduce.out" | tail -n 1 || true)
  [ -n "$doc" ] || doc=$(pb_error invalid_json "babysit-tick.sh: reduce-state.sh exited $rc without a structured error" '{"source":"reduce"}')
  _record_error "$doc"
  printf '%s\n' "$doc"
  exit "$rc"
fi

# --- persist: snapshot first (the state points at it), then state -----------
pb_atomic_write_json "$snapshot_out" cat "$snap" || pb_fail invalid_json "babysit-tick.sh: could not write $snapshot_out" '{"source":"snapshot_out"}'
pb_atomic_write_json "$state_file" cat "$PB_TMPDIR/state.next.json" || pb_fail invalid_json "babysit-tick.sh: could not write $state_file" '{"source":"state"}'
cat "$PB_TMPDIR/result.json"
