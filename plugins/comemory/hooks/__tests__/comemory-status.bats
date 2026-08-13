#!/usr/bin/env bats
# Tests for the comemory-status SessionStart hook. Exercises the REAL comemory
# binary against an isolated temp data dir (COMEMORY_DATA_DIR) — no mocks, no
# PATH stubs. Each test seeds (or leaves empty) a real store and asserts the
# marker the statusline reads.

HOOK="${BATS_TEST_DIRNAME}/../comemory-status.sh"

setup() {
  TMP=$(mktemp -d)                        # before skip, so teardown's rm always has a target
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  ( cd "$TMP" && git init -q )            # repo so git-common-dir resolves
  STORE="$TMP/store"; mkdir -p "$STORE"
  KEY=$(basename "$TMP")                  # repo_key for a plain checkout = basename
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

_run_hook() {
  printf '{"cwd":"%s"}' "$TMP" \
    | COMEMORY_DATA_DIR="$STORE" CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$HOOK"
}

_marker() { cat "$TMP/cfg/comemory-status/$KEY.json" 2>/dev/null; }

@test "comemory-status: writes a count-0 marker for an empty store" {
  _run_hook
  [ "$(_marker | jq -r '.count')" = "0" ]
  [ "$(_marker | jq -r '.repo')" = "$KEY" ]
}

@test "comemory-status: count reflects seeded memories" {
  COMEMORY_DATA_DIR="$STORE" comemory save "race in migrate when run twice" --kind bug --repo "$KEY" >/dev/null 2>&1
  COMEMORY_DATA_DIR="$STORE" comemory save "prefer postgres for analytics" --kind decision --repo "$KEY" >/dev/null 2>&1
  _run_hook
  [ "$(_marker | jq -r '.count')" = "2" ]
}

@test "comemory-status: marker is keyed by the repo name" {
  _run_hook
  [ -f "$TMP/cfg/comemory-status/$KEY.json" ]
}

@test "comemory-status: Codex marker is written under CODEX_HOME" {
  local codex_home="$TMP/codex-home"
  printf '{"cwd":"%s"}' "$TMP" \
    | COMEMORY_DATA_DIR="$STORE" TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$codex_home" \
      bash "$HOOK"
  [ -f "$codex_home/comemory-status/$KEY.json" ]
  [ ! -e "$TMP/cfg/comemory-status/$KEY.json" ]
}

@test "comemory-status: no marker when cwd is not a repo" {
  notrepo=$(mktemp -d)
  run sh -c "printf '{\"cwd\":\"$notrepo\"}' | COMEMORY_DATA_DIR=\"$STORE\" CLAUDE_CONFIG_DIR=\"$TMP/cfg\" bash \"$HOOK\""
  [ "$status" -eq 0 ]
  [ ! -d "$TMP/cfg/comemory-status" ]
  rm -rf "$notrepo"
}
