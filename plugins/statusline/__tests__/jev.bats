#!/usr/bin/env bats
# Exercise the real Jev publisher and both statusline consumers, without API calls.

ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
JEV_HOOK="$ROOT/../jev/hooks/session-start.sh"

setup() {
  TMP=$(mktemp -d)
  unset TOOLU_HOST_OVERRIDE TOOLU_CONFIG_DIR PLUGIN_ROOT
  export CLAUDE_CONFIG_DIR="$TMP/claude profile" CODEX_HOME="$TMP/codex profile"
  export TYPESAFE_API_KEY=statusline-test-key
  mkdir -p "$TMP/workspace"
}

teardown() {
  rm -rf "$TMP"
}

publish_jev() {
  TOOLU_HOST_OVERRIDE="$1" bash "$JEV_HOOK" </dev/null >/dev/null
}

render_claude() {
  jq -nc --arg cwd "$TMP/workspace" '{workspace:{current_dir:$cwd}}' |
    bash "$ROOT/statusline.sh"
}

@test "Jev readiness: Claude renders ready from the published wrapper" {
  publish_jev claude
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *$'\033[32m[JEV:READY]'* ]]
  [[ "$output" != *"$TYPESAFE_API_KEY"* ]]
}

@test "Jev readiness: Codex report uses its published wrapper without lifecycle variables" {
  publish_jev codex
  run bash "$ROOT/scripts/status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Jev: ready"* ]]
  [[ "$output" != *"$TYPESAFE_API_KEY"* ]]
}

@test "Jev readiness: missing or empty key is unavailable with a reason" {
  publish_jev claude
  unset TYPESAFE_API_KEY
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *$'\033[33m[JEV:UNAVAILABLE: missing TYPESAFE_API_KEY]'* ]]

  publish_jev codex
  export TYPESAFE_API_KEY=""
  run bash "$ROOT/scripts/status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Jev: unavailable — missing TYPESAFE_API_KEY"* ]]
}

@test "Jev readiness: malformed credentials are unavailable without exposing their contents" {
  publish_jev claude
  for TYPESAFE_API_KEY in $'secret\nvalue' $'secret\rvalue'; do
    export TYPESAFE_API_KEY
    run render_claude
    [ "$status" -eq 0 ]
    [[ "$output" == *"[JEV:UNAVAILABLE: invalid TYPESAFE_API_KEY]"* ]]
    [[ "$output" != *"secret"* ]]
    [[ "$output" != *"value"* ]]
  done
}

@test "Jev readiness: unpublished plugin is omitted on both hosts" {
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" != *"[JEV:"* ]]
  run bash "$ROOT/scripts/status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  [[ "$output" != *"Jev:"* ]]
}

@test "Jev readiness: broken wrapper symlink is unavailable" {
  mkdir -p "$CLAUDE_CONFIG_DIR/jev"
  ln -s "$TMP/removed/jev.sh" "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *"[JEV:UNAVAILABLE: missing executable wrapper]"* ]]
}

@test "Jev readiness: a non-executable wrapper or directory cannot be ready" {
  mkdir -p "$CLAUDE_CONFIG_DIR/jev"
  cp "$ROOT/../jev/skills/jev/scripts/jev.sh" "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  chmod -x "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *"[JEV:UNAVAILABLE: missing executable wrapper]"* ]]

  rm "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  mkdir "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *"[JEV:UNAVAILABLE: missing executable wrapper]"* ]]
}

@test "Jev readiness: collector reports missing curl using only local prerequisites" {
  publish_jev claude
  mkdir "$TMP/bin"
  for tool in jq git basename dirname; do
    ln -s "$(command -v "$tool")" "$TMP/bin/$tool"
  done
  run env PATH="$TMP/bin" /bin/bash "$ROOT/scripts/collect-status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  jq -e '.jev == {status:"unavailable",reason:"missing curl"}' <<<"$output"
}

@test "Jev readiness: hosts never fall back to another profile's wrapper" {
  publish_jev claude
  run bash "$ROOT/scripts/status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  [[ "$output" != *"Jev:"* ]]

  rm "$CLAUDE_CONFIG_DIR/jev/jev.sh"
  publish_jev codex
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" != *"[JEV:"* ]]
}

@test "Jev readiness: explicit config root takes priority on both hosts" {
  export TOOLU_CONFIG_DIR="$TMP/explicit profile"
  publish_jev claude
  run render_claude
  [ "$status" -eq 0 ]
  [[ "$output" == *"[JEV:READY]"* ]]
  run bash "$ROOT/scripts/status.sh" "$TMP/workspace"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Jev: ready"* ]]
}

@test "Jev readiness: Claude still renders readiness when payload has no workspace" {
  publish_jev claude
  run bash "$ROOT/statusline.sh" <<<'{}'
  [ "$status" -eq 0 ]
  [[ "$output" == *"[JEV:READY]"* ]]
}
