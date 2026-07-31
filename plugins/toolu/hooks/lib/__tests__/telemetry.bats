#!/usr/bin/env bats
# Tests for hooks/lib/telemetry.sh — the workflow telemetry append engine.
# Real temp git repos, real state files, no mocked commands.

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -b main -q
    git config user.email "t@example.com"
    git config user.name "Tester"
    git commit -q --allow-empty -m init
    git checkout -q -b feat/x
  )

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../telemetry.sh
  . "$REPO_ROOT/hooks/lib/telemetry.sh"

  # Isolated config dirs so the merged toolu config starts empty (telemetry
  # defaults to enabled) unless a test writes its own project config.
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$REPO"
  unset TOOLU_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME TELEMETRY_DIR
  mkdir -p "$HOME/.claude" "$REPO/.claude"
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'

  TELEMETRY_FILE="$REPO/.claude/tmp/telemetry/feat_x.jsonl"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

@test "telemetry_append: writes exactly one line of valid JSON with v/t/branch/event and merged extra keys" {
  ( cd "$REPO" && telemetry_append "$REPO" "step_run" '{"step_id":"s1","status":"green"}' )

  [ -f "$TELEMETRY_FILE" ]
  [ "$(wc -l < "$TELEMETRY_FILE" | tr -d ' ')" = "1" ]

  jq -e . "$TELEMETRY_FILE" >/dev/null
  [ "$(jq -r '.v' "$TELEMETRY_FILE")" = "1" ]
  [ "$(jq -r '.branch' "$TELEMETRY_FILE")" = "feat/x" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "step_run" ]
  [ "$(jq -r '.step_id' "$TELEMETRY_FILE")" = "s1" ]
  [ "$(jq -r '.status' "$TELEMETRY_FILE")" = "green" ]
  # ISO-8601 UTC timestamp.
  [[ "$(jq -r '.t' "$TELEMETRY_FILE")" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]
}

@test "telemetry_append: no extra-json arg defaults to {} and still writes the base fields" {
  ( cd "$REPO" && telemetry_append "$REPO" "gate_fail" )
  [ -f "$TELEMETRY_FILE" ]
  jq -e . "$TELEMETRY_FILE" >/dev/null
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "gate_fail" ]
}

@test "telemetry_append: two appends produce two lines, each independently valid JSON" {
  ( cd "$REPO" && telemetry_append "$REPO" "step_run" '{"step_id":"s1"}' )
  ( cd "$REPO" && telemetry_append "$REPO" "step_run" '{"step_id":"s2"}' )

  [ "$(wc -l < "$TELEMETRY_FILE" | tr -d ' ')" = "2" ]
  local l1 l2
  l1=$(sed -n '1p' "$TELEMETRY_FILE")
  l2=$(sed -n '2p' "$TELEMETRY_FILE")
  [ "$(jq -c . <<< "$l1")" != "" ]
  [ "$(jq -c . <<< "$l2")" != "" ]
  [ "$(jq -r '.step_id' <<< "$l1")" = "s1" ]
  [ "$(jq -r '.step_id' <<< "$l2")" = "s2" ]
}

@test "telemetry_append: TELEMETRY_DIR override is honored" {
  local altdir="$TMP/alt-telemetry"
  ( cd "$REPO" && TELEMETRY_DIR="$altdir" telemetry_append "$REPO" "push_check" '{"result":"allow"}' )

  [ ! -f "$TELEMETRY_FILE" ]
  [ -f "$altdir/feat_x.jsonl" ]
  [ "$(jq -r '.result' "$altdir/feat_x.jsonl")" = "allow" ]
}

@test "telemetry_append: detached HEAD writes nothing and still exits 0" {
  ( cd "$REPO" && git checkout -q "$(git rev-parse HEAD)" )

  run bash -c "cd '$REPO' && . '$REPO_ROOT/hooks/lib/telemetry.sh' && telemetry_append '$REPO' delegation"
  [ "$status" -eq 0 ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "telemetry_append: telemetry.enabled=false in project config is a no-op" {
  echo '{"version":1,"telemetry":{"enabled":false}}' > "$REPO/.claude/toolu.config.json"

  run bash -c "
    export CLAUDE_PROJECT_DIR='$REPO' HOME='$HOME'
    cd '$REPO'
    . '$REPO_ROOT/hooks/lib/telemetry.sh'
    telemetry_append '$REPO' docs_nudge
  "
  [ "$status" -eq 0 ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "telemetry_append: jq absent from PATH writes nothing and still exits 0" {
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash git cat printf mkdir tr date sed wc; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c"
  done

  run env -i HOME="$HOME" CLAUDE_PROJECT_DIR="$REPO" PATH="$STUB" bash -c "
    cd '$REPO'
    . '$REPO_ROOT/hooks/lib/telemetry.sh'
    telemetry_append '$REPO' delegation
  "
  [ "$status" -eq 0 ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}

@test "telemetry_append: invalid extra JSON exits 0, warns on stderr, appends no line" {
  run bash -c "cd '$REPO' && . '$REPO_ROOT/hooks/lib/telemetry.sh' && telemetry_append '$REPO' step_run 'not-json{'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"telemetry:"* ]]
  [[ "$output" == *"malformed"* ]]
  [ ! -f "$TELEMETRY_FILE" ]
}

@test "telemetry_append: extra JSON cannot override the v/t/branch/event protocol fields" {
  ( cd "$REPO" && telemetry_append "$REPO" "step_run" \
    '{"v":9,"branch":"spoof","event":"x","t":"1999-01-01T00:00:00Z"}' )

  [ -f "$TELEMETRY_FILE" ]
  jq -e . "$TELEMETRY_FILE" >/dev/null
  # Protocol fields win regardless of what extra tried to smuggle in.
  [ "$(jq -r '.v' "$TELEMETRY_FILE")" = "1" ]
  [ "$(jq -r '.branch' "$TELEMETRY_FILE")" = "feat/x" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "step_run" ]
  [[ "$(jq -r '.t' "$TELEMETRY_FILE")" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]
  [ "$(jq -r '.t' "$TELEMETRY_FILE")" != "1999-01-01T00:00:00Z" ]
}

@test "telemetry_append: assembled line over 3900 bytes warns on stderr, appends nothing, still exits 0" {
  local pad extra
  pad=$(printf 'x%.0s' $(seq 1 4000))
  extra=$(jq -cn --arg pad "$pad" '{pad: $pad}')

  run telemetry_append "$REPO" step_run "$extra"
  [ "$status" -eq 0 ]
  [[ "$output" == *"telemetry:"* ]]
  [[ "$output" == *"3900"* ]]
  [ ! -f "$TELEMETRY_FILE" ]
}
