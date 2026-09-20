#!/usr/bin/env bash
# common.sh — shared plumbing for the pr-babysit helper scripts.
#
# Sourced, never run. Provides: plugin-root resolution from this file's own
# location (no environment variable names the root), dependency checks, a
# structured-error emitter with the closed exit-code map, an atomic JSON
# writer, and the single EXIT handler every entrypoint installs via pb_init.
#
# Portability contract (see docs/toolu/specs/2026-09-19-pr-babysit-helper-design.md):
# bash 3.2 — no mapfile/readarray, no declare -A, no wait -n, no ${var,,}.

# Closed error-code → exit-code map. Adding a code is a schema change.
#   usage 2 · gh_unavailable/jq_required/api_error/invalid_json/head_moved/
#   state_malformed/slot_mismatch 3 · duplicate_reply 4 · resolve_unconfirmed 5
#   locked 75 (EX_TEMPFAIL)
pb_exit_code() {
  case "$1" in
    usage) echo 2 ;;
    duplicate_reply) echo 4 ;;
    resolve_unconfirmed) echo 5 ;;
    locked) echo 75 ;;
    gh_unavailable|jq_required|api_error|invalid_json|head_moved|state_malformed|slot_mismatch) echo 3 ;;
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
  local code="$1" message="$2" extra="${3:-{\}}"
  jq -nc --arg code "$code" --arg message "$message" --argjson extra "$extra" \
    '{version:1, errors:[({code:$code, message:$message} + $extra)]}'
}

# pb_fail CODE MESSAGE [EXTRA_JSON] -> emit the error and exit with its code.
pb_fail() {
  pb_error "$@"
  exit "$(pb_exit_code "$1")"
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
pb_atomic_write_json() {
  local target="$1"; shift
  local dir tmp
  dir=$(dirname "$target")
  mkdir -p "$dir" || return 1
  tmp=$(mktemp "$dir/.$(basename "$target").tmp.XXXXXX") || return 1
  if ! "$@" >"$tmp"; then
    rm -f "$tmp"; return 1
  fi
  if ! pb_json_valid "$tmp"; then
    rm -f "$tmp"; return 1
  fi
  mv -f "$tmp" "$target" || { rm -f "$tmp"; return 1; }
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
  [ -n "${PB_TMPDIR:-}" ] && [ -d "${PB_TMPDIR:-}" ] && rm -rf "$PB_TMPDIR"
  return "$rc"
}

# pb_init — install the EXIT handler and forward TERM/INT into it.
pb_init() {
  trap pb_on_exit EXIT
  trap 'exit 143' TERM
  trap 'exit 130' INT
}
