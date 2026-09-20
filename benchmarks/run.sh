#!/usr/bin/env bash
# run.sh — benchmarks harness entry point.
#
# Dispatches to per-mechanism cases by tier. The deterministic tier (retrieval)
# is hermetic and CI-safe; the live tier (whole-session) needs an API key / the
# claude CLI and is run manually. Live cases that have not been built yet are
# skipped with a notice rather than failing the run.
set -u

_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'USAGE'
usage: run.sh --tier <deterministic|live|both> [--mechanism <name|all>]
       run.sh --validate <result.json>

mechanisms:
  retrieval                         deterministic tier (hermetic, CI)
  whole-session                     live tier (manual; needs API key / claude CLI)
  all                               every mechanism for the selected tier
USAGE
}

main() {
  local tier="both" mechanism="all" validate=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --tier)      tier="${2:-}";      shift 2 ;;
      --mechanism) mechanism="${2:-}"; shift 2 ;;
      --validate)  validate="${2:-}";  shift 2 ;;
      -h|--help)   usage; return 0 ;;
      *) echo "run.sh: unknown arg: $1" >&2; usage >&2; return 2 ;;
    esac
  done

  if [ -n "$validate" ]; then
    # shellcheck source=/dev/null
    source "$_dir/lib/result.sh"
    bench_result_validate "$validate"; return $?
  fi

  case "$tier" in deterministic|live|both) ;; *) echo "run.sh: bad --tier: $tier" >&2; return 2 ;; esac

  local rc=0
  if [ "$tier" = "deterministic" ] || [ "$tier" = "both" ]; then
    case "$mechanism" in
      all|retrieval) bash "$_dir/cases/retrieval/run.sh" || rc=1 ;;
    esac
  fi

  if [ "$tier" = "live" ] || [ "$tier" = "both" ]; then
    local live_sel="$mechanism"
    [ "$live_sel" = "all" ] && live_sel="whole-session"
    local m
    for m in $live_sel; do
      case "$m" in
        whole-session)
          if [ -x "$_dir/cases/$m/run.sh" ]; then
            bash "$_dir/cases/$m/run.sh" || rc=1
          else
            echo "run.sh: live case not yet available: $m (skipping)" >&2
          fi ;;
        retrieval) : ;;
        *) echo "run.sh: unknown mechanism: $m (live tier: whole-session)" >&2; rc=2 ;;
      esac
    done
  fi
  return $rc
}

main "$@"
