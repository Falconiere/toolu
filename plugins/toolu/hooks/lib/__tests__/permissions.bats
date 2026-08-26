#!/usr/bin/env bats
# Tests for hooks/lib/permissions.sh.

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  cd "$REPO"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  git commit -q --allow-empty -m init

  export HOME="$TMP/home"
  export TOOLU_PROJECT_DIR="$REPO"
  export CLAUDE_PROJECT_DIR="$REPO"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE PLUGIN_ROOT
  mkdir -p "$HOME/.claude" "$REPO/.claude"

  SETTINGS="$REPO/.claude/settings.local.json"
  SENTINEL="$REPO/.claude/tmp/.permissions-written"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../permissions.sh
  . "$REPO_ROOT/hooks/lib/permissions.sh"
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

config() {
  printf '%s' "$1" > "$REPO/.claude/toolu.config.json"
  TOOLU_CFG_LOADED=0
}

@test "a fresh repo gets the blanket allowlist and a sentinel" {
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.permissions.allow | index("Bash(*)") != null' "$SETTINGS")" = "true" ]
  [ "$(jq -r '.permissions.allow | index("Edit") != null' "$SETTINGS")" = "true" ]
  [ "$(jq -r '.permissions.allow | index("Write") != null' "$SETTINGS")" = "true" ]
  [ -f "$SENTINEL" ]
}

@test "the committed toolu.config.json is not touched" {
  config '{"version":1}'
  before=$(cat "$REPO/.claude/toolu.config.json")
  toolu_permissions_autowrite "$REPO"
  [ "$(cat "$REPO/.claude/toolu.config.json")" = "$before" ]
}

@test "a second run changes nothing" {
  toolu_permissions_autowrite "$REPO"
  before=$(cat "$SETTINGS")
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 1 ]
  [ "$(cat "$SETTINGS")" = "$before" ]
}

@test "a hand-deleted rule stays deleted" {
  toolu_permissions_autowrite "$REPO"
  jq '.permissions.allow = ["Edit"]' "$SETTINGS" > "$SETTINGS.new" && mv "$SETTINGS.new" "$SETTINGS"
  # Returns 1 (skipped, sentinel present) — which is the point of the test.
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 1 ]
  [ "$(jq -r '.permissions.allow | index("Bash(*)") // "gone"' "$SETTINGS")" = "gone" ]
}

@test "existing allow entries and a deny block survive the merge" {
  cat > "$SETTINGS" <<'JSON'
{"permissions":{"allow":["Bash(ls:*)","Bash(pwd)"],"deny":["Bash(rm -rf /)"]},"env":{"FOO":"bar"}}
JSON
  toolu_permissions_autowrite "$REPO"
  [ "$(jq -r '.permissions.allow[0]' "$SETTINGS")" = "Bash(ls:*)" ]
  [ "$(jq -r '.permissions.allow[1]' "$SETTINGS")" = "Bash(pwd)" ]
  [ "$(jq -r '.permissions.deny[0]' "$SETTINGS")" = "Bash(rm -rf /)" ]
  [ "$(jq -r '.env.FOO' "$SETTINGS")" = "bar" ]
  [ "$(jq -r '.permissions.allow | index("Bash(*)") != null' "$SETTINGS")" = "true" ]
}

@test "an entry that is already present is not duplicated" {
  printf '%s' '{"permissions":{"allow":["Bash(*)"]}}' > "$SETTINGS"
  toolu_permissions_autowrite "$REPO"
  [ "$(jq -r '[.permissions.allow[] | select(. == "Bash(*)")] | length' "$SETTINGS")" = "1" ]
}

@test "a malformed settings file is left byte-identical" {
  printf 'this is not json' > "$SETTINGS"
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 1 ]
  [ "$(cat "$SETTINGS")" = "this is not json" ]
  [ ! -f "$SENTINEL" ]
}

@test "autoAllow false skips the write" {
  config '{"version":1,"permissions":{"autoAllow":false}}'
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 1 ]
  [ ! -f "$SETTINGS" ]
}

@test "a non-git directory is skipped" {
  plain="$TMP/plain"
  mkdir -p "$plain"
  run toolu_permissions_autowrite "$plain"
  [ "$status" -eq 1 ]
}

@test "codex is skipped" {
  export TOOLU_HOST_OVERRIDE=codex
  run toolu_permissions_autowrite "$REPO"
  [ "$status" -eq 1 ]
  [ ! -f "$SETTINGS" ]
}

@test "a configured allow list replaces the default" {
  config '{"version":1,"permissions":{"allow":["Bash(git:*)","Bash(gh:*)"]}}'
  toolu_permissions_autowrite "$REPO"
  [ "$(jq -r '.permissions.allow | join(",")' "$SETTINGS")" = "Bash(git:*),Bash(gh:*)" ]
}

@test "a configured allow list of the wrong shape falls back to the default" {
  config '{"version":1,"permissions":{"allow":"Bash(*)"}}'
  toolu_permissions_autowrite "$REPO"
  [ "$(jq -r '.permissions.allow | index("Bash(*)") != null' "$SETTINGS")" = "true" ]
}

@test "the summary names what was added and where" {
  run toolu_permissions_autowrite "$REPO"
  [[ "$output" == *"Bash(*)"* ]]
  [[ "$output" == *"settings.local.json"* ]]
  [[ "$output" == *"one time only"* ]]
}

@test "the written settings file is valid JSON" {
  toolu_permissions_autowrite "$REPO"
  run jq -e . "$SETTINGS"
  [ "$status" -eq 0 ]
}
