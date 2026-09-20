#!/usr/bin/env bats
# Real-data tests for scripts/babysit-tick.sh — the orchestrator both hosts
# call. Offline block replays captured snapshots (--snapshot-in) through the
# real lock, reducer and atomic persist; the live block runs a full tick
# against merged Falconiere/toolu#115. No mocks.

SCRIPTS="${BATS_TEST_DIRNAME}/.."
SNAP="${BATS_TEST_DIRNAME}/fixtures/snapshots"
NOW=2026-09-19T12:00:00Z
LATER=2026-09-19T12:03:00Z

setup() {
  TMP=$(mktemp -d)
  STATE="$TMP/pr-babysit-falconiere-toolu-165.json"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

tick() { bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-165.json" "$@"; }

@test "first tick from a captured snapshot: result on stdout, state + snapshot beside it, nothing else" {
  out=$(tick --now "$NOW")
  jq -e '.version == 1' <<<"$out" >/dev/null
  [ "$(jq -r .decision <<<"$out")" = escalate ]
  [ "$(jq -r '.reasons[0].code' <<<"$out")" = pr_merged ]
  [ "$(jq -r .statePath <<<"$out")" = "$STATE" ]
  [ "$(jq -r .snapshotPath <<<"$out")" = "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json" ]
  jq -e '.version == 2' "$STATE" >/dev/null
  [ "$(jq -r .totalTicks "$STATE")" = 1 ]
  [ "$(jq -r .lastGoodSnapshot "$STATE")" = "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json" ]
  cmp "$SNAP/toolu-165.json" "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json"
  [ "$(ls -A "$TMP" | sort | paste -sd ' ' -)" = "pr-babysit-falconiere-toolu-165.json pr-babysit-falconiere-toolu-165.snapshot.json" ]
  # Lock released.
  [ ! -d "$STATE.lock" ]
}

@test "second tick resumes the slot: unchanged, idleStreak 1, totalTicks 2" {
  tick --now "$NOW" >/dev/null
  out=$(tick --now "$LATER")
  [ "$(jq -r .changed <<<"$out")" = false ]
  [ "$(jq -r .backoff.idleStreak <<<"$out")" = 1 ]
  [ "$(jq -r .totalTicks "$STATE")" = 2 ]
  [ "$(jq -r .lastUpdate "$STATE")" = "$LATER" ]
}

@test "--snapshot-out overrides the snapshot location and the result reports it" {
  out=$(tick --now "$NOW" --snapshot-out "$TMP/elsewhere/snap.json")
  [ "$(jq -r .snapshotPath <<<"$out")" = "$TMP/elsewhere/snap.json" ]
  [ -f "$TMP/elsewhere/snap.json" ]
  [ "$(jq -r .lastGoodSnapshot "$STATE")" = "$TMP/elsewhere/snap.json" ]
}

@test "usage: missing/invalid arguments exit 2 before touching anything" {
  run bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165
  [ "$status" -eq 2 ]; [ "$(jq -r '.errors[0].code' <<<"$output")" = usage ]
  run bash "$SCRIPTS/babysit-tick.sh" --repo nope --pr 165 --state-file "$STATE"
  [ "$status" -eq 2 ]
  run tick --now "not-a-time"
  [ "$status" -eq 2 ]
  run tick --page-size x
  [ "$status" -eq 2 ]
  run bash "$SCRIPTS/babysit-tick.sh" --wat
  [ "$status" -eq 2 ]
  [ -z "$(ls -A "$TMP")" ]
}

@test "AC-9: malformed state (not JSON / version 99) → exit 3 state_malformed, bytes unchanged, no snapshot written" {
  printf 'not json' >"$STATE"
  run tick --now "$NOW"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = state_malformed ]
  [ "$(cat "$STATE")" = "not json" ]
  [ ! -e "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json" ]
  printf '{"version":99,"repo":"Falconiere/toolu","number":165}' >"$STATE"
  cp "$STATE" "$TMP/before"
  run tick --now "$NOW"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = state_malformed ]
  [ "$(jq -r '.errors[0].version' <<<"$output")" = 99 ]
  cmp "$TMP/before" "$STATE"
  [ ! -d "$STATE.lock" ]
}

@test "slot identity: a state file for another PR, or a snapshot for another PR, is refused with slot_mismatch" {
  tick --now "$NOW" >/dev/null
  run bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 115 --state-file "$STATE" --snapshot-in "$SNAP/toolu-115.json"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = slot_mismatch ]
  [ "$(jq -r '.errors[0].state.number' <<<"$output")" = 165 ]
  [ "$(jq -r .totalTicks "$STATE")" = 1 ]
  run bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-115.json"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = slot_mismatch ]
  [ "$(jq -r '.errors[0].source' <<<"$output")" = snapshot ]
}

@test "AC-8: a live holder makes the tick exit 75 locked, without writing" {
  bash -c ". '$SCRIPTS/lib/common.sh'; . '$SCRIPTS/lib/lock.sh'; pb_init; pb_lock_acquire '$STATE'; sleep 3" &
  holder=$!
  for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do [ -f "$STATE.lock/since" ] && break; sleep 0.1; done
  run tick --now "$NOW"
  [ "$status" -eq 75 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = locked ]
  [ "$(jq -r '.errors[0].pid' <<<"$output")" = "$(cat "$STATE.lock/pid")" ]
  [ ! -e "$STATE" ]
  kill "$holder" 2>/dev/null || true; wait "$holder" 2>/dev/null || true
}

@test "AC-8: two ticks racing on one slot — every exit is 0 or 75, the state is intact and counts only the winners" {
  # Both start together; the lock serialises them. Whether the loser sees 75
  # or runs after the winner depends on scheduling, so assert the invariants
  # rather than one interleaving.
  for i in 1 2; do
    ( rc=0; tick --now "$NOW" >"$TMP/out.$i" 2>/dev/null || rc=$?; echo "$rc" >"$TMP/rc.$i" ) &
  done
  wait
  rc1=$(cat "$TMP/rc.1"); rc2=$(cat "$TMP/rc.2")
  [[ "$rc1" = 0 || "$rc1" = 75 ]]
  [[ "$rc2" = 0 || "$rc2" = 75 ]]
  [[ "$rc1" = 0 || "$rc2" = 0 ]]
  wins=0; [ "$rc1" = 0 ] && wins=$((wins + 1)); [ "$rc2" = 0 ] && wins=$((wins + 1))
  jq -e '.version == 2' "$STATE" >/dev/null
  [ "$(jq -r .totalTicks "$STATE")" = "$wins" ]
  [ ! -d "$STATE.lock" ]
  [ -z "$(ls -A "$TMP" | grep -E '\.tmp\.')" ]
  for i in 1 2; do
    if [ "$(cat "$TMP/rc.$i")" = 75 ]; then [ "$(jq -r '.errors[0].code' "$TMP/out.$i")" = locked ]; fi
  done
}

@test "AC-10: SIGTERM at random points across 20 ticks never leaves a partial state, a temp file, or a live lock" {
  tick --now "$NOW" >/dev/null
  cp "$STATE" "$TMP/old.json"
  # The complete new state this tick would write (same --now → byte-identical).
  cp "$STATE" "$TMP/probe.json"
  bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$TMP/probe.json" --snapshot-in "$SNAP/toolu-165.json" --now "$LATER" >/dev/null
  cp "$TMP/probe.json" "$TMP/new.json"
  for i in $(seq 1 20); do
    cp "$TMP/old.json" "$STATE"
    # Background the script itself (not a bats function: that would fork a
    # wrapper subshell, and the kill would orphan the real tick instead).
    bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --snapshot-in "$SNAP/toolu-165.json" --now "$LATER" >/dev/null 2>&1 &
    victim=$!
    # 0–100 ms, spread with the iteration so kills land in different phases.
    sleep "0.0$((i % 10))$((i % 3))"
    kill -TERM "$victim" 2>/dev/null || true
    wait "$victim" 2>/dev/null || true
    jq -e '.version == 2' "$STATE" >/dev/null
    # Either the untouched old state, or the complete new one. `new.json` was
    # produced through a differently named state file, so its lastGoodSnapshot
    # path is the one field that legitimately differs.
    if ! cmp -s "$STATE" "$TMP/old.json"; then
      cmp <(jq -S 'del(.lastGoodSnapshot)' "$STATE") <(jq -S 'del(.lastGoodSnapshot)' "$TMP/new.json")
      [ "$(jq -r .lastGoodSnapshot "$STATE")" = "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json" ]
    fi
    [ -z "$(ls -A "$TMP" | grep -E '\.tmp\.')" ]
    [ ! -d "$STATE.lock" ]
  done
}

@test "collection failure with a prior state: error passed through, state kept, pr.lastError stamped" {
  tick --now "$NOW" >/dev/null
  run env GH_HOST=127.0.0.1:1 GH_TOKEN=x PB_GH_BACKOFF='0 0 0' bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$STATE" --now "$LATER"
  [ "$status" -eq 3 ]
  doc=$(printf '%s\n' "$output" | grep '^{')
  [ "$(jq -r '.errors[0].code' <<<"$doc")" = api_error ]
  [ "$(jq -r '.errors[0].attempts' <<<"$doc")" = 3 ]
  [ "$(jq -r .totalTicks "$STATE")" = 1 ]
  [ "$(jq -r .pr.lastError.code "$STATE")" = api_error ]
  [ "$(jq -r .pr.lastError.at "$STATE")" = "$LATER" ]
  [ "$(jq -r .lastGoodSnapshot "$STATE")" = "$TMP/pr-babysit-falconiere-toolu-165.snapshot.json" ]
  [ ! -d "$STATE.lock" ]
}

@test "AC-1 (live): a real tick on merged #115 writes state exactly at --state-file and reports 17 threads" {
  [ "${PR_BABYSIT_LIVE:-}" = 1 ] || skip "set PR_BABYSIT_LIVE=1 to run against api.github.com"
  gh auth status >/dev/null 2>&1 || skip "gh not authenticated"
  state="$TMP/pr-babysit-falconiere-toolu-115.json"
  out=$(bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 115 --state-file "$state")
  jq -e '.version == 1' <<<"$out" >/dev/null
  [ "$(jq -r .decision <<<"$out")" = escalate ]
  [ "$(jq -r '.reasons[0].code' <<<"$out")" = pr_merged ]
  [ "$(jq '.reasons | length' <<<"$out")" -ge 2 ]
  [ "$(jq -r .threads.total <<<"$out")" = 17 ]
  jq -e '.version == 2' "$state" >/dev/null
  [ "$(jq -r .pr.key "$state")" = "Falconiere/toolu#115" ]
  [ "$(ls -A "$TMP" | sort | paste -sd ' ' -)" = "pr-babysit-falconiere-toolu-115.json pr-babysit-falconiere-toolu-115.snapshot.json" ]
}
