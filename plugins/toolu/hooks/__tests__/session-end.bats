#!/usr/bin/env bats
# session-end.sh is the Stop hook. The comemory reminder is OPT-IN (default off):
# it emits only when hooks.session-end is explicitly true.

SCRIPT="${BATS_TEST_DIRNAME}/../session-end.sh"

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$TMP/proj"
  # Pin the config dir so the maintenance stamp path is deterministic regardless
  # of any ambient CLAUDE_CONFIG_DIR in the runner's environment.
  export CLAUDE_CONFIG_DIR="$HOME/.claude"
  mkdir -p "$HOME/.claude" "$CLAUDE_PROJECT_DIR/.claude"
  # Isolate comemory's real data writes to a throwaway dir.
  export COMEMORY_DATA_DIR="$TMP/cm"
  mkdir -p "$COMEMORY_DATA_DIR"
  # Pre-stamp today's maintenance at its real location (toolu config dir,
  # NOT the comemory data dir) so the output-focused tests do NOT trigger the
  # detached maintenance run; the dedicated tests below clear it first.
  STAMP="$HOME/.claude/toolu/.comemory-last-maintain"
  mkdir -p "$HOME/.claude/toolu"
  printf '%s' "$(date -u +%Y%m%d)" > "$STAMP"
}

teardown() {
  # Maintenance is detached (backgrounded + disowned); a run may still hold the
  # throwaway data dir. Tolerate a racing writer — never fail the test on rm.
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP" 2>/dev/null
  return 0
}

_enable() { echo '{"version":1,"hooks":{"session-end":true}}' > "$HOME/.claude/toolu.config.json"; }

@test "session-end is silent by default (opt-in, no config)" {
  run bash -c "'$SCRIPT' < /dev/null"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "session-end stays silent when explicitly disabled" {
  echo '{"version":1,"hooks":{"session-end":false}}' > "$HOME/.claude/toolu.config.json"
  run bash -c "'$SCRIPT' < /dev/null"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "session-end emits valid JSON with systemMessage when enabled" {
  _enable
  output=$(bash -c "'$SCRIPT' < /dev/null")
  echo "$output" | jq -e '.systemMessage' >/dev/null
}

@test "session-end (enabled) does not emit unrecognized stopReason field" {
  _enable
  output=$(bash -c "'$SCRIPT' < /dev/null")
  ! echo "$output" | jq -e '.stopReason' >/dev/null
}

@test "session-end (enabled) output does not leak source-repo names" {
  _enable
  run bash -c "'$SCRIPT' < /dev/null"
  ! echo "$output" | grep -qE 'yamless|routo|/Volumes/Projects/(routo|yamless)'
}

# Autonomous maintenance is opt-OUT (default on): the once-per-day stamp file is
# written even with no config. The stamp lives in the toolu config dir (not
# the comemory data dir). Clear setup's pre-stamp so the throttle does not
# short-circuit; the stamp is written synchronously before the detached run, so
# this assertion never races the background job.
@test "session-end: autonomous comemory maintenance runs by default (stamp written)" {
  command -v comemory >/dev/null 2>&1 || skip "comemory not installed"
  STAMP="$HOME/.claude/toolu/.comemory-last-maintain"
  rm -f "$STAMP"
  run bash -c "'$SCRIPT' < /dev/null"
  [ "$status" -eq 0 ]
  [ -f "$STAMP" ]
}

@test "session-end: Codex maintenance stamp uses CODEX_HOME" {
  command -v comemory >/dev/null 2>&1 || skip "comemory not installed"
  local codex_home="$TMP/codex"
  local codex_stamp="$codex_home/toolu/.comemory-last-maintain"
  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$codex_home" TOOLU_PROJECT_DIR="$TMP/proj" \
    bash -c "'$SCRIPT' < /dev/null"
  [ "$status" -eq 0 ]
  [ -f "$codex_stamp" ]
}

# Contract fix: hooks.session-end:false disables BOTH the reminder and the
# autonomous maintenance — no stamp, no store mutation. Clear the pre-stamp so a
# present stamp can only mean maintenance ran (it must not).
@test "session-end: hooks.session-end=false disables maintenance (no stamp)" {
  command -v comemory >/dev/null 2>&1 || skip "comemory not installed"
  echo '{"version":1,"hooks":{"session-end":false}}' > "$HOME/.claude/toolu.config.json"
  STAMP="$HOME/.claude/toolu/.comemory-last-maintain"
  rm -f "$STAMP"
  run bash -c "'$SCRIPT' < /dev/null"
  [ "$status" -eq 0 ]
  [ ! -f "$STAMP" ]
}
