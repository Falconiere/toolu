#!/usr/bin/env bats
# Real-data tests for scripts/lib/common.sh and scripts/lib/lock.sh: real temp
# dirs, real subprocesses, real pids. No mocks.

LIB="${BATS_TEST_DIRNAME}/../lib"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Drive the libs through a fresh bash so trap/exit semantics are the ones a
# real entrypoint gets.
with_libs() {
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/lock.sh'; $1"
}

@test "pb_plugin_root resolves to plugins/pr-babysit from the lib's own location" {
  out=$(with_libs 'pb_plugin_root')
  [ "$(basename "$out")" = "pr-babysit" ]
  [ -f "$out/scripts/parse-verdict.sh" ]
}

@test "pb_exit_code maps the closed error-code set" {
  [ "$(with_libs 'pb_exit_code usage')" = 2 ]
  [ "$(with_libs 'pb_exit_code duplicate_reply')" = 4 ]
  [ "$(with_libs 'pb_exit_code resolve_unconfirmed')" = 5 ]
  [ "$(with_libs 'pb_exit_code locked')" = 75 ]
  for c in gh_unavailable jq_required api_error invalid_json head_moved state_malformed slot_mismatch; do
    [ "$(with_libs "pb_exit_code $c")" = 3 ]
  done
}

@test "pb_error emits a versioned errors[] document with extra fields merged" {
  out=$(with_libs 'pb_error api_error "boom" "{\"attempts\":3}"')
  [ "$(jq -r '.version' <<<"$out")" = 1 ]
  [ "$(jq -r '.errors[0].code' <<<"$out")" = api_error ]
  [ "$(jq -r '.errors[0].message' <<<"$out")" = boom ]
  [ "$(jq -r '.errors[0].attempts' <<<"$out")" = 3 ]
}

@test "pb_fail exits with the mapped code after emitting the error" {
  run with_libs 'pb_fail state_malformed "bad"'
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = state_malformed ]
  run with_libs 'pb_fail locked "held"'
  [ "$status" -eq 75 ]
}

@test "pb_atomic_write_json writes valid JSON and leaves no temp file" {
  with_libs "pb_atomic_write_json '$TMP/s.json' jq -nc '{a:1}'"
  [ "$(jq -r .a "$TMP/s.json")" = 1 ]
  [ -z "$(ls -A "$TMP" | grep -v '^s.json$')" ]
}

@test "pb_atomic_write_json: writer that fails midway leaves target byte-identical and no temp" {
  printf '{"keep":true}\n' >"$TMP/s.json"
  cp "$TMP/s.json" "$TMP/before"
  # jq syntax error → nonzero exit after possibly partial output.
  run with_libs "pb_atomic_write_json '$TMP/s.json' jq -n '{a:1} |'"
  [ "$status" -ne 0 ]
  cmp "$TMP/before" "$TMP/s.json"
  [ -z "$(ls -A "$TMP" | grep -v -e '^s.json$' -e '^before$')" ]
}

@test "pb_atomic_write_json: writer that exits 0 with invalid JSON is rejected" {
  printf '{"keep":true}\n' >"$TMP/s.json"
  run with_libs "pb_atomic_write_json '$TMP/s.json' printf 'not json'"
  [ "$status" -ne 0 ]
  [ "$(cat "$TMP/s.json")" = '{"keep":true}' ]
  [ -z "$(ls -A "$TMP" | grep -v '^s.json$')" ]
}

@test "pb_atomic_write_json creates the parent directory" {
  with_libs "pb_atomic_write_json '$TMP/a/b/s.json' jq -nc '{}'"
  [ -f "$TMP/a/b/s.json" ]
}

@test "lock: acquire records pid and since, release removes the dir" {
  with_libs "pb_lock_acquire '$TMP/slot.json'; [ -f '$TMP/slot.json.lock/pid' ]; [ \"\$(cat '$TMP/slot.json.lock/pid')\" = \"\$\$\" ]; [ -f '$TMP/slot.json.lock/since' ]; pb_lock_release; [ ! -d '$TMP/slot.json.lock' ]"
}

@test "lock: a live holder makes a second taker return 75 with holder info" {
  # Holder: a real background process that keeps the lock for 3s.
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/lock.sh'; pb_lock_acquire '$TMP/slot.json'; sleep 3" &
  holder=$!
  # Wait until the lock dir is fully written.
  for _ in 1 2 3 4 5 6 7 8 9 10; do [ -f "$TMP/slot.json.lock/since" ] && break; sleep 0.1; done
  run with_libs "pb_lock_acquire '$TMP/slot.json' || { echo \"rc=\$? pid=\$PB_LOCK_HOLDER_PID\"; exit 75; }"
  [ "$status" -eq 75 ]
  [[ "$output" == *"rc=75 pid=$(cat "$TMP/slot.json.lock/pid")"* ]]
  kill "$holder" 2>/dev/null || true
  wait "$holder" 2>/dev/null || true
}

@test "lock: pb_lock_fail emits the locked error with numeric pid/since" {
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/lock.sh'; pb_lock_acquire '$TMP/slot.json'; sleep 3" &
  holder=$!
  for _ in 1 2 3 4 5 6 7 8 9 10; do [ -f "$TMP/slot.json.lock/since" ] && break; sleep 0.1; done
  run with_libs "pb_lock_acquire '$TMP/slot.json' || pb_lock_fail '$TMP/slot.json'"
  [ "$status" -eq 75 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = locked ]
  [ "$(jq -r '.errors[0].pid | type' <<<"$output")" = number ]
  [ "$(jq -r '.errors[0].since | type' <<<"$output")" = number ]
  kill "$holder" 2>/dev/null || true
  wait "$holder" 2>/dev/null || true
}

@test "lock: a dead holder pid is stale and reclaimed" {
  (true) & dead=$!; wait "$dead"
  mkdir "$TMP/slot.json.lock"
  echo "$dead" >"$TMP/slot.json.lock/pid"
  date +%s >"$TMP/slot.json.lock/since"
  run with_libs "pb_lock_acquire '$TMP/slot.json' && cat '$TMP/slot.json.lock/pid'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"reclaiming stale lock"* ]]
  [ "$(cat "$TMP/slot.json.lock/pid")" != "$dead" ]
}

@test "lock: a live holder older than the stale threshold is reclaimed" {
  bash -c "sleep 5" & old=$!
  mkdir "$TMP/slot.json.lock"
  echo "$old" >"$TMP/slot.json.lock/pid"
  echo $(( $(date +%s) - 700 )) >"$TMP/slot.json.lock/since"
  run with_libs "pb_lock_acquire '$TMP/slot.json'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"reclaiming stale lock"* ]]
  kill "$old" 2>/dev/null || true; wait "$old" 2>/dev/null || true
}

@test "lock: a half-written lock (no pid file) is reclaimed" {
  mkdir "$TMP/slot.json.lock"
  run with_libs "pb_lock_acquire '$TMP/slot.json'"
  [ "$status" -eq 0 ]
}

@test "lock: two concurrent takers — exactly one wins" {
  for i in 1 2; do
    bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/lock.sh'; if pb_lock_acquire '$TMP/slot.json'; then sleep 1; echo win; else echo lose; fi" >"$TMP/out.$i" 2>/dev/null &
  done
  wait
  wins=$(cat "$TMP/out.1" "$TMP/out.2" | grep -c '^win$')
  loses=$(cat "$TMP/out.1" "$TMP/out.2" | grep -c '^lose$')
  [ "$wins" -eq 1 ]
  [ "$loses" -eq 1 ]
}

@test "pb_init: SIGTERM releases the lock and removes the scratch dir through the EXIT trap" {
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/lock.sh'; pb_init; pb_mktmpdir; echo \"\$PB_TMPDIR\" > '$TMP/tmpdir'; pb_lock_acquire '$TMP/slot.json'; sleep 10" &
  victim=$!
  for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do [ -f "$TMP/slot.json.lock/since" ] && [ -f "$TMP/tmpdir" ] && break; sleep 0.1; done
  [ -d "$TMP/slot.json.lock" ]
  scratch=$(cat "$TMP/tmpdir")
  [ -d "$scratch" ]
  kill -TERM "$victim"
  wait "$victim" 2>/dev/null || true
  [ ! -d "$TMP/slot.json.lock" ]
  [ ! -d "$scratch" ]
}

# --- pb_retry_on_rc (the head-moved policy in collect-pr.sh) -----------------

# A real command whose exit code is driven by a counter file, so each call is
# observable: exits with the N-th code from its argument list on the N-th
# call, or 0 once the list runs out.
retry_probe() {
  printf '%s\n' '#!/usr/bin/env bash' \
    'dir=$1; shift' \
    'n=$(cat "$dir/count" 2>/dev/null || echo 0); n=$((n + 1)); echo "$n" >"$dir/count"' \
    'i=1; for code in "$@"; do [ "$i" -eq "$n" ] && exit "$code"; i=$((i + 1)); done' \
    'exit 0' >"$TMP/probe.sh"
  chmod +x "$TMP/probe.sh"
}

@test "pb_retry_on_rc: the retry code earns exactly one more attempt, then the second result stands" {
  retry_probe
  # moved once, then consistent → success after 2 calls
  run with_libs "pb_retry_on_rc 10 2 '$TMP/probe.sh' '$TMP' 10 0"
  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/count")" = 2 ]
  # The log names the retried command ($1 after the two policy arguments).
  [[ "$output" == *"attempt 1 of 2 returned 10; retrying $TMP/probe.sh"* ]]
  # moved twice → the retry code comes back after exactly 2 calls (collect-pr.sh maps it to head_moved)
  rm -f "$TMP/count"
  run with_libs "pb_retry_on_rc 10 2 '$TMP/probe.sh' '$TMP' 10 10 0"
  [ "$status" -eq 10 ]
  [ "$(cat "$TMP/count")" = 2 ]
}

@test "pb_retry_on_rc: any other failure propagates on the first call, success never retries" {
  retry_probe
  run with_libs "pb_retry_on_rc 10 2 '$TMP/probe.sh' '$TMP' 3 0"
  [ "$status" -eq 3 ]
  [ "$(cat "$TMP/count")" = 1 ]
  rm -f "$TMP/count"
  run with_libs "pb_retry_on_rc 10 2 '$TMP/probe.sh' '$TMP' 0 10"
  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/count")" = 1 ]
}
