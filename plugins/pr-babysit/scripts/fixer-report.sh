#!/usr/bin/env bash
# fixer-report.sh — the one command a babysit fixer agent runs when it is done.
#
# Usage: fixer-report.sh <report-file> done|failed [--note <text>]
#
# Writes {"version":1,"status":…,"note":…,"at":…} atomically to the report file
# named in the fixer's brief. The dispatcher reads it when the agent settles;
# commits are read from git, never from this file.
# Exit: 0 · 2 usage.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"

report="${1:-}"; status="${2:-}"
[ $# -ge 2 ] && shift 2 || shift $#
note=""; has_note=false
while [ $# -gt 0 ]; do
  case "$1" in
    --note) note="${2:-}"; has_note=true; shift 2 ;;
    *) pb_fail usage "fixer-report.sh: unknown argument: $1" ;;
  esac
done
[ -n "$report" ] || pb_fail usage "fixer-report.sh: <report-file> done|failed [--note <text>]"
[[ "$status" =~ ^(done|failed)$ ]] || pb_fail usage "fixer-report.sh: status must be done or failed"
pb_require jq
pb_init

pb_atomic_write_json "$report" jq -nc --arg s "$status" --arg n "$note" --argjson hasNote "$has_note" --arg at "$(pb_now)" \
  '{version: 1, status: $s, note: (if $hasNote then $n else null end), at: $at}' \
  || pb_fail usage "fixer-report.sh: cannot write $report"
