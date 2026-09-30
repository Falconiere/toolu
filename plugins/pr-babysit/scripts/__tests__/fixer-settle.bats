#!/usr/bin/env bats
# Real-data tests for scripts/fixer-report.sh and scripts/lib/fixer.sh (AC-8):
# report files written by the shipped fixer-report.sh, and the provider
# throttle messages epic-orchestrator's hosts.test.ts pins for HOST_LIMIT,
# read from that file so both suites share one list. No mocks.

SCRIPTS="${BATS_TEST_DIRNAME}/.."
LIB="$SCRIPTS/lib"
EPIC_TEST="$(cd "${BATS_TEST_DIRNAME}/../../../epic-orchestrator/scripts/__tests__" && pwd -P)/hosts.test.ts"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

with_fixer() {
  bash -c "set -euo pipefail; . '$LIB/fixer-compat.sh'; . '$LIB/hosts.sh'; . '$LIB/fixer.sh'; $1"
}

# settle REPORT_FILE OUTPUT_TEXT -> the settle outcome
settle() {
  printf '%s' "$2" >"$TMP/pane.txt"
  with_fixer "pb_fixer_settle '$1' \"\$(cat '$TMP/pane.txt')\""
}

# The throttle lines epic's "limit pattern" test asserts HOST_LIMIT matches.
limit_lines() {
  awk '/for \(const line of \[/{f=1; next} /\]\) \{/{f=0} f' "$EPIC_TEST" | sed -E 's/^[[:space:]]*"(.*)",?$/\1/'
}

@test "AC-8: fixer-report.sh done writes a versioned report; settle -> done" {
  bash "$SCRIPTS/fixer-report.sh" "$TMP/r.json" done --note "guarded the empty case"
  [ "$(jq -c '{version, status, note}' "$TMP/r.json")" = '{"version":1,"status":"done","note":"guarded the empty case"}' ]
  jq -e '.at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T")' "$TMP/r.json" >/dev/null
  [ "$(settle "$TMP/r.json" "")" = done ]
}

@test "AC-8: a failed report -> reported_failed, even when the pane shows a limit message" {
  bash "$SCRIPTS/fixer-report.sh" "$TMP/r.json" failed
  [ "$(jq -r '.note' "$TMP/r.json")" = null ]
  [ "$(settle "$TMP/r.json" "Claude usage limit reached")" = reported_failed ]
}

@test "AC-8: no report + each provider throttle line from epic hosts.test.ts -> host_limited" {
  n=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    [ "$(settle "$TMP/absent.json" "$line")" = host_limited ] || { echo "not matched: $line" >&2; return 1; }
    n=$((n + 1))
  done < <(limit_lines)
  [ "$n" -eq 4 ]
}

@test "AC-8: no report + ordinary output -> no_report; an unparsable report counts as none" {
  [ "$(settle "$TMP/absent.json" "All 34 tests passed")" = no_report ]
  printf '{"status": "do' >"$TMP/broken.json"
  [ "$(settle "$TMP/broken.json" "All 34 tests passed")" = no_report ]
}

@test "AC-8 boundary: fixer-report.sh rejects an unknown status (exit 2) and writes nothing" {
  run bash "$SCRIPTS/fixer-report.sh" "$TMP/r.json" maybe
  [ "$status" -eq 2 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = usage ]
  [ ! -e "$TMP/r.json" ]
  run bash "$SCRIPTS/fixer-report.sh"
  [ "$status" -eq 2 ]
  [ "$(jq -r '.errors[0].message' <<<"$output")" = "fixer-report.sh: <report-file> done|failed [--note <text>]" ]
  # One argument: the report file is read; the missing status is what is named.
  run bash "$SCRIPTS/fixer-report.sh" "$TMP/r.json"
  [ "$status" -eq 2 ]
  [ "$(jq -r '.errors[0].message' <<<"$output")" = "fixer-report.sh: status must be done or failed" ]
  [ ! -e "$TMP/r.json" ]
}

@test "AC-8 boundary: agent names are herdr-valid, stable per slot/round/group, distinct across slots" {
  a=$(with_fixer 'pb_fixer_agent_name falconiere-toolu-42 2 1')
  [[ "$a" =~ ^[a-z][a-z0-9_-]{0,31}$ ]]
  [[ "$a" =~ ^pb-[0-9a-f]{6}-r2g1$ ]]
  [ "$(with_fixer 'pb_fixer_agent_name falconiere-toolu-42 2 1')" = "$a" ]
  [ "$(with_fixer 'pb_fixer_agent_name falconiere-comemory-42 2 1')" != "$a" ]
  [ "$(with_fixer 'pb_fixer_agent_name falconiere-toolu-42 2 2')" != "$a" ]
}

@test "brief and report paths sit beside the slot state file" {
  [ "$(with_fixer 'pb_fixer_brief_path /tmp/pr-babysit-falconiere-toolu-42.json 2 1')" = /tmp/pr-babysit-falconiere-toolu-42.fixer-r2g1.md ]
  [ "$(with_fixer 'pb_fixer_report_path /tmp/pr-babysit-falconiere-toolu-42.json 2 1')" = /tmp/pr-babysit-falconiere-toolu-42.fixer-r2g1.report.json ]
}
