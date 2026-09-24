#!/usr/bin/env bash
# Worker status report for the epic orchestrator.
# Usage: report.sh <status-file> <phase> [--pr N] [--note "text"]
# Writes {phase, pr, note, updated_at, history[]} atomically; history keeps every transition.
set -euo pipefail

PHASES="spec spec-review plan plan-review execution pr-open babysit rebasing ready needs-human failed"

usage() { echo "usage: report.sh <status-file> <phase> [--pr N] [--note text]; phases: $PHASES" >&2; exit 2; }

[[ $# -ge 2 ]] || usage
file=$1 phase=$2
shift 2
[[ " $PHASES " == *" $phase "* ]] || { echo "unknown phase: $phase" >&2; usage; }

pr="" note=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --pr) pr=${2:?--pr needs a number}; shift 2 ;;
    --note) note=${2:?--note needs text}; shift 2 ;;
    *) usage ;;
  esac
done
[[ -z $pr || $pr =~ ^[0-9]+$ ]] || { echo "--pr must be a number" >&2; exit 2; }

mkdir -p "$(dirname "$file")"
prev='{}'
[[ -s $file ]] && prev=$(jq -c '.' "$file" 2>/dev/null || echo '{}')
now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

jq --arg phase "$phase" --arg pr "$pr" --arg note "$note" --arg now "$now" '
  (.pr // null) as $oldpr
  | .phase = $phase
  | .pr = (if $pr == "" then $oldpr else ($pr | tonumber) end)
  | .note = (if $note == "" then null else $note end)
  | .updated_at = $now
  | .history = ((.history // []) + [{phase: $phase, at: $now, note: .note}])
' <<<"$prev" >"$file.tmp"
mv "$file.tmp" "$file"
echo "reported $phase${pr:+ (PR #$pr)}"
