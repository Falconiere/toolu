#!/usr/bin/env bash
# stats.sh — entry point for the /stats report. Parses options, scans the
# transcripts (memoized), aggregates, and renders. Read-only except for the
# per-session rollup cache it maintains under $CLAUDE_CONFIG_DIR/stats/.
#
#   stats.sh [--today|--week|--all] [--project P] [--model M] [--session ID]
#            [--this-session] [--since YYYY-MM-DD] [--limit N]
#            [--plan pro|max5|max20] [--billing api|subscription|both]
#            [--json] [--html] [--rescan]
#
# Cost figures are sticker-price estimates, not a bill. With --plan (or a plan in
# ~/.claude/stats.conf) the report also shows the Claude-subscription lens:
# month-to-date API-equivalent value, savings vs the flat fee, and weekly quota.
set -u

# jq is the one hard dependency. Check before touching anything external so the
# missing-jq path stays self-contained (echo is a builtin; no PATH needed).
command -v jq >/dev/null 2>&1 || {
  echo "stats: jq not found — install jq to see usage stats."
  exit 0
}

LIB="$(cd "${0%/*}/lib" && pwd)" || { echo "stats: cannot locate lib dir." >&2; exit 1; }
# shellcheck source=/dev/null
source "$LIB/scan.sh"
# shellcheck source=/dev/null
source "$LIB/aggregate.sh"
# shellcheck source=/dev/null
source "$LIB/render.sh"
# shellcheck source=/dev/null
source "$LIB/render-html.sh"
# shellcheck source=/dev/null
source "$LIB/config.sh"
# shellcheck source=/dev/null
source "$LIB/billing.sh"

# Print the leading comment block (after the shebang) as help, robust to its
# length — stops at the first non-comment line.
usage() {
  awk 'NR==1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
}

stats_load_config                 # file defaults; CLI flags below override them
WINDOW=all; SINCE=""; THIS=0
while [ $# -gt 0 ]; do
  case "$1" in
    --today) WINDOW=today ;;
    --week)  WINDOW=week ;;
    --all)   WINDOW=all ;;
    --json)        export STATS_OUTPUT=json ;;
    --html)        export STATS_OUTPUT=html ;;
    --rescan)      export STATS_FORCE_RESCAN=1 ;;
    --this-session) THIS=1 ;;
    --project) shift; export STATS_PROJECT="${1:-}" ;;
    --model)   shift; export STATS_MODEL="${1:-}" ;;
    --session) shift; export STATS_SESSION="${1:-}" ;;
    --limit)   shift; export STATS_LIMIT="${1:-10}" ;;
    --plan)    shift; export STATS_PLAN="${1:-}" ;;
    --billing) shift; export STATS_BILLING="${1:-}" ;;
    --since)   shift; SINCE="${1:-}" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "stats: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done
export STATS_WINDOW="$WINDOW"

if [ "$THIS" -eq 1 ]; then
  # Current session = newest transcript under this dir's project; always fresh.
  t="$(stats_current_transcript "$PWD")" || {
    echo "stats: no session transcript found for this directory."
    exit 0
  }
  printf '[%s]\n' "$(stats_rollup_session "$t")" | stats_aggregate | stats_attach_billing | stats_render
  exit 0
fi

rolls="$(stats_scan_all)"
if [ -n "$SINCE" ]; then
  # Keep sessions whose most recent active day is on/after --since.
  rolls="$(printf '%s' "$rolls" | jq --arg s "$SINCE" \
    'map(select((([.by_day|keys[]] | max) // "0") >= $s))')"
fi
printf '%s' "$rolls" | stats_aggregate | stats_attach_billing | stats_render
