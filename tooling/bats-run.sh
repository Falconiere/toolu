#!/usr/bin/env bash
# Run the bats suite, in parallel where the host can.
#
# The suite is ~1600 tests and almost all of its wall-clock is process spawns:
# every test builds a real git repo and every hook invocation shells out to jq
# a dozen times. Serially that is ~10 minutes; across 8 cores it is a fraction
# of that. bats needs GNU parallel (or shenwei356/rush) for --jobs, so this
# resolves what is actually available and degrades to a serial run rather than
# failing when neither is installed.
#
# Files run in parallel; tests WITHIN a file stay serial. Several suites share
# per-file state on purpose — a throwaway comemory repo label, a cwd, a fixture
# repo built in setup — and parallelising inside a file would race them.
#
# Usage: bash tooling/bats-run.sh [path...]     (default: plugins benchmarks tooling)
# Env:   BATS_JOBS=N              force the job count; 1 forces a serial run.
#        BATS_RUN_PRINT_PLAN=1    print the command that would run, then exit.
#                                 Lets the runner's decisions be tested without
#                                 nesting one bats run inside another.

set -euo pipefail

paths=("$@")
# These three are the whole suite. benchmarks/ used to be missing from the local
# default while CI ran it, so 40 tests only ever failed in CI.
[ "${#paths[@]}" -gt 0 ] || paths=(plugins benchmarks tooling)

command -v bats >/dev/null 2>&1 || {
  echo "bats-run: bats is not installed" >&2
  exit 127
}

# Job count: explicit override, else one per core, else a conservative 4.
jobs="${BATS_JOBS:-}"
if [ -z "$jobs" ]; then
  jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)
fi
case "$jobs" in
  ''|*[!0-9]*) jobs=1 ;;
esac

parallel_ok=0
if [ "$jobs" -gt 1 ]; then
  if command -v parallel >/dev/null 2>&1 || command -v rush >/dev/null 2>&1; then
    parallel_ok=1
  else
    echo "bats-run: GNU parallel not found; running serially (install it for a ~3x faster suite)" >&2
  fi
fi

# `bats -r` on a path with no suites is a confusing error; discovery being
# broken is worth naming directly.
found=$(find "${paths[@]}" -name '*.bats' -print -quit 2>/dev/null || true)
if [ -z "$found" ]; then
  echo "bats-run: no .bats suites found under ${paths[*]} — test discovery is broken" >&2
  exit 1
fi

if [ "$parallel_ok" -eq 1 ]; then
  echo "bats-run: $jobs jobs, files in parallel, tests within a file serial" >&2
  if [ -n "${BATS_RUN_PRINT_PLAN:-}" ]; then
    echo "bats -r ${paths[*]} --jobs $jobs --no-parallelize-within-files"
    exit 0
  fi
  exec bats -r "${paths[@]}" --jobs "$jobs" --no-parallelize-within-files
fi

if [ -n "${BATS_RUN_PRINT_PLAN:-}" ]; then
  echo "bats -r ${paths[*]}"
  exit 0
fi

exec bats -r "${paths[@]}"
