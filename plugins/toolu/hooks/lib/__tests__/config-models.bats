#!/usr/bin/env bats
# Tests for the model-tier router in hooks/lib/config.sh (toolu_model,
# toolu_model_valid). Real config files on disk, real jq. No mocks.

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME
  mkdir -p "$HOME/.claude" "$CLAUDE_PROJECT_DIR/.claude"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../config.sh
  . "$REPO_ROOT/hooks/lib/config.sh"
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

@test "toolu_model: built-in defaults with no config on disk" {
  [ "$(toolu_model mechanical)"     = "haiku" ]
  [ "$(toolu_model exploration)"    = "sonnet" ]
  [ "$(toolu_model implementation)" = "sonnet" ]
  [ "$(toolu_model review)"         = "sonnet" ]
  [ "$(toolu_model synthesis)"      = "opus" ]
  [ "$(toolu_model architecture)"   = "opus" ]
}

@test "toolu_model: the ladder is ordered cheapest to most capable" {
  # The whole point of the router: mechanical work must not land on the tier
  # that architecture work lands on.
  [ "$(toolu_model mechanical)" != "$(toolu_model exploration)" ]
  [ "$(toolu_model exploration)" != "$(toolu_model architecture)" ]
  [ "$(toolu_model mechanical)" != "$(toolu_model architecture)" ]
}

@test "toolu_model: user config remaps a class" {
  echo '{"version":1,"models":{"review":"opus"}}' > "$HOME/.claude/toolu.config.json"
  [ "$(toolu_model review)" = "opus" ]
}

@test "toolu_model: unremapped classes keep their defaults" {
  echo '{"version":1,"models":{"review":"opus"}}' > "$HOME/.claude/toolu.config.json"
  [ "$(toolu_model mechanical)" = "haiku" ]
  [ "$(toolu_model exploration)" = "sonnet" ]
}

@test "toolu_model: project config overrides user config" {
  echo '{"version":1,"models":{"mechanical":"opus"}}'   > "$HOME/.claude/toolu.config.json"
  echo '{"version":1,"models":{"mechanical":"sonnet"}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  [ "$(toolu_model mechanical)" = "sonnet" ]
}

@test "toolu_model: 'inherit' is a routable override value" {
  echo '{"version":1,"models":{"synthesis":"inherit"}}' > "$HOME/.claude/toolu.config.json"
  [ "$(toolu_model synthesis)" = "inherit" ]
}

@test "toolu_model: unrecognized alias falls back to the default and warns" {
  echo '{"version":1,"models":{"review":"gpt-9"}}' > "$HOME/.claude/toolu.config.json"
  run toolu_model review
  [ "$status" -eq 0 ]
  # stdout+stderr are merged by `run`: the default must be present, and the
  # rejection must be reported rather than silently swallowed.
  echo "$output" | grep -q 'sonnet'
  echo "$output" | grep -q 'is not a model alias'
}

@test "toolu_model: a non-string value is rejected, not stringified into a model name" {
  echo '{"version":1,"models":{"review":42}}' > "$HOME/.claude/toolu.config.json"
  run toolu_model review
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'sonnet'
  ! echo "$output" | grep -qx '42'
}

@test "toolu_model: an explicit null falls through to the default" {
  echo '{"version":1,"models":{"review":null}}' > "$HOME/.claude/toolu.config.json"
  run toolu_model review
  [ "$status" -eq 0 ]
  [ "$output" = "sonnet" ]
}

@test "toolu_model: unknown class returns 1 and prints no alias" {
  run toolu_model nonsense
  [ "$status" -eq 1 ]
  echo "$output" | grep -q "unknown model class"
  ! echo "$output" | grep -qE '^(haiku|sonnet|opus)$'
}

@test "toolu_model: malformed config JSON falls back to defaults" {
  printf '{' > "$HOME/.claude/toolu.config.json"
  run toolu_model architecture
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'opus'
}

@test "toolu_model: models.enabled is not mistaken for a class override" {
  # `enabled` is the injection switch and shares the block with class keys;
  # turning it off must not perturb any tier.
  echo '{"version":1,"models":{"enabled":false}}' > "$HOME/.claude/toolu.config.json"
  [ "$(toolu_model mechanical)" = "haiku" ]
  [ "$(toolu_model architecture)" = "opus" ]
  run toolu_enabled models enabled
  [ "$status" -eq 1 ]
}

@test "toolu_model: injection is enabled by default and when set true" {
  run toolu_enabled models enabled
  [ "$status" -eq 0 ]
  echo '{"version":1,"models":{"enabled":true}}' > "$HOME/.claude/toolu.config.json"
  TOOLU_CFG_LOADED=0; TOOLU_CFG_JSON='{}'
  run toolu_enabled models enabled
  [ "$status" -eq 0 ]
}

@test "toolu_model_valid: accepts every routable alias, rejects the rest" {
  for a in haiku sonnet opus fable inherit; do
    run toolu_model_valid "$a"
    [ "$status" -eq 0 ]
  done
  for a in "" "claude-opus-5" "frontier" "Sonnet"; do
    run toolu_model_valid "$a"
    [ "$status" -eq 1 ]
  done
}
