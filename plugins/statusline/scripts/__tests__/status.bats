#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
COLLECT="$ROOT/scripts/collect-status.sh"
STATUS="$ROOT/scripts/status.sh"

setup() {
  TMP=$(mktemp -d)
  git init -q -b main "$TMP/repo"
  git -C "$TMP/repo" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "collector reads Codex gate state and repository status without Claude fallback" {
  mkdir -p "$TMP/repo/.codex/tmp" "$TMP/repo/.claude/tmp"
  printf '%s\n' '{"status":"failing","reason":"codex failure"}' \
    > "$TMP/repo/.codex/tmp/quality-gate-status.json"
  printf '%s\n' '{"status":"passing"}' \
    > "$TMP/repo/.claude/tmp/quality-gate-status.json"
  git -C "$TMP/repo" add .codex .claude
  git -C "$TMP/repo" -c user.email=t@t -c user.name=t commit -q -m state
  touch "$TMP/repo/untracked.txt"

  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex-home" \
    bash "$COLLECT" "$TMP/repo"

  [ "$status" -eq 0 ]
  echo "$output" | jq -e '
    .host == "codex" and .branch == "main" and .gate.status == "failing" and
    .gate.reason == "codex failure" and .working_tree.untracked == 1'
}

@test "collector reads the host-native comemory marker when present" {
  key=$(basename "$TMP/repo")
  mkdir -p "$TMP/codex-home/comemory-status"
  printf '{"repo":"%s","count":7}\n' "$key" \
    > "$TMP/codex-home/comemory-status/$key.json"

  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex-home" \
    bash "$COLLECT" "$TMP/repo"

  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.comemory_count == 7'
}

@test "Codex status report omits unavailable Claude model context and account fields" {
  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex-home" \
    bash "$STATUS" "$TMP/repo"

  [ "$status" -eq 0 ]
  [[ "$output" == *"Host: Codex"* ]]
  [[ "$output" == *"Branch: main"* ]]
  [[ "$output" == *"Quality gate: no recorded state"* ]]
  [[ "$output" != *"Model:"* ]]
  [[ "$output" != *"Context:"* ]]
  [[ "$output" != *"Account:"* ]]
}

@test "Codex status script selects Codex paths without lifecycle environment variables" {
  mkdir -p "$TMP/repo/.codex/tmp" "$TMP/repo/.claude/tmp" "$TMP/home"
  printf '%s\n' '{"status":"failing","reason":"codex state"}' \
    > "$TMP/repo/.codex/tmp/quality-gate-status.json"
  printf '%s\n' '{"status":"passing"}' \
    > "$TMP/repo/.claude/tmp/quality-gate-status.json"

  run bash -c 'unset TOOLU_HOST_OVERRIDE PLUGIN_ROOT CODEX_HOME CLAUDE_CONFIG_DIR; HOME="$1" bash "$2" "$3"' \
    _ "$TMP/home" "$STATUS" "$TMP/repo"

  [ "$status" -eq 0 ]
  [[ "$output" == *"Host: Codex"* ]]
  [[ "$output" == *"Quality gate: failing — codex state"* ]]
}
