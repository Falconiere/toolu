#!/usr/bin/env bats
# Real-data tests for scripts/lib/gh.sh. The transport is the real `gh`
# binary: the refused-host cases point it at a closed loopback port; the
# classification cases feed it responses captured from GitHub on 2026-09-19
# (fixtures/gh/*). 5xx/429/rate-limit inputs use gh's documented stderr shape
# `gh: <reason> (HTTP <status>)` because no live upstream produces them on
# demand — they are strings under test, not stand-ins for a component.

LIB="${BATS_TEST_DIRNAME}/../lib"
FX="${BATS_TEST_DIRNAME}/fixtures/gh"

setup() {
  TMP=$(mktemp -d)
  command -v gh >/dev/null 2>&1 || skip "gh not installed"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

with_libs() {
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/gh.sh'; $1"
}

@test "pb_run_with_timeout returns the command's own exit code and captures stdout" {
  run with_libs "rc=0; pb_run_with_timeout 5 '$TMP/out' '$TMP/err' bash -c 'echo hi; exit 7' || rc=\$?; echo rc=\$rc"
  [ "$status" -eq 0 ]
  [[ "$output" == *"rc=7"* ]]
  [ "$(cat "$TMP/out")" = hi ]
}

@test "pb_run_with_timeout kills a slow command and returns 124 in bounded time" {
  start=$(date +%s)
  run with_libs "rc=0; pb_run_with_timeout 1 '$TMP/out' '$TMP/err' sleep 5 || rc=\$?; echo rc=\$rc"
  end=$(date +%s)
  [[ "$output" == *"rc=124"* ]]
  [ $((end - start)) -lt 4 ]
  grep -q 'timed out after 1s' "$TMP/err"
}

@test "classify: captured connection-refused stderr is transient" {
  : >"$TMP/out"
  [ "$(with_libs "pb_gh_classify 1 '$FX/refused.err' '$TMP/out'")" = transient ]
}

@test "classify: captured REST 404 is permanent" {
  [ "$(with_libs "pb_gh_classify 1 '$FX/404.err' '$FX/404.out'")" = permanent ]
}

@test "classify: captured GraphQL NOT_FOUND (data present + errors[]) is permanent" {
  [ "$(with_libs "pb_gh_classify 1 '$FX/graphql-notfound.err' '$FX/graphql-notfound.out'")" = permanent ]
}

@test "classify: watchdog rc 124 is transient; rc 0 is ok" {
  : >"$TMP/out"; : >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 124 '$TMP/err' '$TMP/out'")" = transient ]
  [ "$(with_libs "pb_gh_classify 0 '$TMP/err' '$TMP/out'")" = ok ]
}

@test "classify: gh-format 502, 429, and 403 rate-limit are transient; plain 403 and 401 are permanent" {
  : >"$TMP/out"
  echo 'gh: Bad Gateway (HTTP 502)' >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 1 '$TMP/err' '$TMP/out'")" = transient ]
  echo 'gh: Too Many Requests (HTTP 429)' >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 1 '$TMP/err' '$TMP/out'")" = transient ]
  echo 'gh: API rate limit exceeded for user ID 1. (HTTP 403)' >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 1 '$TMP/err' '$TMP/out'")" = transient ]
  echo 'gh: Resource not accessible by integration (HTTP 403)' >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 1 '$TMP/err' '$TMP/out'")" = permanent ]
  echo 'gh: Bad credentials (HTTP 401)' >"$TMP/err"
  [ "$(with_libs "pb_gh_classify 1 '$TMP/err' '$TMP/out'")" = permanent ]
}

@test "pb_gh_json_ok rejects captured GraphQL errors[] and accepts clean objects/arrays" {
  run with_libs "pb_gh_json_ok '$FX/graphql-notfound.out'"
  [ "$status" -ne 0 ]
  echo '{"data":{"x":1}}' >"$TMP/ok.json"
  with_libs "pb_gh_json_ok '$TMP/ok.json'"
  echo '[{"id":1}]' >"$TMP/arr.json"
  with_libs "pb_gh_json_ok '$TMP/arr.json'"
  echo 'nope' >"$TMP/bad.json"
  run with_libs "pb_gh_json_ok '$TMP/bad.json'"
  [ "$status" -ne 0 ]
}

@test "pb_gh against a refused loopback host retries 3 times (2s + 4s waits) and stays under 60s (AC-12)" {
  start=$(date +%s)
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/gh.sh'; pb_gh '$TMP/out' api repos/Falconiere/toolu/pulls/115 || { echo \"attempts=\$PB_GH_ATTEMPTED class=\$PB_GH_CLASS\"; exit 9; }"
  end=$(date +%s)
  [ "$status" -eq 9 ]
  [[ "$output" == *"attempts=3 class=transient"* ]]
  [ $((end - start)) -ge 6 ]
  [ $((end - start)) -lt 60 ]
  [ "$(grep -c 'retrying in' <<<"$output")" -eq 2 ]
}

@test "pb_gh_fail emits api_error with source, attempts and the last message" {
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x PB_GH_BACKOFF='0 0 0' bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/gh.sh'; pb_gh '$TMP/out' api repos/x/y/pulls/1 || pb_gh_fail pr"
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].code' <<<"$doc")" = api_error ]
  [ "$(jq -r '.errors[0].source' <<<"$doc")" = pr ]
  [ "$(jq -r '.errors[0].attempts' <<<"$doc")" = 3 ]
  [[ "$(jq -r '.errors[0].lastMessage' <<<"$doc")" == *"connection refused"* ]]
}

@test "pb_gh does not retry a permanent error (live 404, gated)" {
  [ "${PR_BABYSIT_LIVE:-}" = 1 ] || skip "set PR_BABYSIT_LIVE=1 to run against api.github.com"
  gh auth status >/dev/null 2>&1 || skip "gh not authenticated"
  run with_libs "pb_gh '$TMP/out' api repos/Falconiere/toolu/pulls/999999 || { echo \"attempts=\$PB_GH_ATTEMPTED class=\$PB_GH_CLASS\"; exit 9; }"
  [ "$status" -eq 9 ]
  [[ "$output" == *"attempts=1 class=permanent"* ]]
}

@test "pb_gh succeeds and leaves no .err file (live, gated)" {
  [ "${PR_BABYSIT_LIVE:-}" = 1 ] || skip "set PR_BABYSIT_LIVE=1 to run against api.github.com"
  gh auth status >/dev/null 2>&1 || skip "gh not authenticated"
  with_libs "pb_gh '$TMP/out' api repos/Falconiere/toolu/pulls/115 --jq .number"
  [ "$(cat "$TMP/out")" = 115 ]
  [ ! -f "$TMP/out.err" ]
}
