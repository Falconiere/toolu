#!/usr/bin/env bats
# Tests for hooks/lib/config.sh.

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME
  mkdir -p "$HOME/.claude" "$CLAUDE_PROJECT_DIR/.claude"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../config.sh
  . "$REPO_ROOT/hooks/lib/config.sh"
  # Reset every loader-state variable so test order cannot leak: cache flag,
  # jq-presence cache, and the in-memory merged config.
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

@test "enabled by default when no config files exist" {
  run toolu_enabled skills comemory
  [ "$status" -eq 0 ]
}

@test "user config disables skill" {
  echo '{"version":1,"skills":{"comemory":false}}' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled skills comemory
  [ "$status" -eq 1 ]
}

@test "project config overrides user config" {
  echo '{"version":1,"skills":{"comemory":false}}' > "$HOME/.claude/toolu.config.json"
  echo '{"version":1,"skills":{"comemory":true}}'  > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_enabled skills comemory
  [ "$status" -eq 0 ]
}

@test "unrelated category remains enabled" {
  echo '{"version":1,"skills":{"comemory":false}}' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled hooks session-start
  [ "$status" -eq 0 ]
}

@test "malformed user JSON falls back to defaults" {
  printf '{' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled skills comemory
  [ "$status" -eq 0 ]
}

@test "comemory_state returns 'disabled' when skills.comemory=false" {
  echo '{"version":1,"skills":{"comemory":false}}' > "$HOME/.claude/toolu.config.json"
  run toolu_comemory_state
  [ "$status" -eq 0 ]
  [ "$output" = "disabled" ]
}

@test "user cfg path honors TOOLU_CONFIG_DIR" {
  run env TOOLU_CONFIG_DIR="$TMP/cfg" bash -c '
    . '"$REPO_ROOT/hooks/lib/config.sh"'; _toolu_user_cfg
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/cfg/toolu.config.json" ]
}

@test "user cfg path honors TOOLU_CONFIG_DIR over CLAUDE_CONFIG_DIR" {
  run env TOOLU_CONFIG_DIR="$TMP/cfg" CLAUDE_CONFIG_DIR="$TMP/claude" bash -c '
    . '"$REPO_ROOT/hooks/lib/config.sh"'; _toolu_user_cfg
  '
  [ "$status" -eq 0 ]
  [ "$output" = "$TMP/cfg/toolu.config.json" ]
}

@test "project cfg path honors TOOLU_PROJECT_CONFIG_DIRNAME" {
  mkdir -p "$CLAUDE_PROJECT_DIR/.alt"
  echo '{"version":1,"skills":{"comemory":false}}' > "$CLAUDE_PROJECT_DIR/.alt/toolu.config.json"
  run env TOOLU_PROJECT_CONFIG_DIRNAME=".alt" bash -c '
    . '"$REPO_ROOT/hooks/lib/config.sh"'; toolu_enabled skills comemory
  '
  [ "$status" -eq 1 ]
}

@test "comemory_state returns 'missing' when enabled but CLI absent" {
  # Use a subshell with env -i so the PATH override stays scoped to the
  # child process; the bats `run` function would otherwise mutate the
  # outer shell's PATH for the remainder of this test.
  run env -i HOME="$HOME" PATH=/usr/bin:/bin bash -c \
    ". \"$REPO_ROOT/hooks/lib/config.sh\"; toolu_comemory_state"
  [ "$status" -eq 0 ]
  [ "$output" = "missing" ]
}


@test "toolu_enabled_explicit: disabled by default (no config)" {
  run toolu_enabled_explicit hooks session-end
  [ "$status" -eq 1 ]
}

@test "toolu_enabled_explicit: disabled when key absent but config exists" {
  echo '{"version":1,"skills":{"comemory":true}}' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled_explicit hooks session-end
  [ "$status" -eq 1 ]
}

@test "toolu_enabled_explicit: enabled only when explicitly true" {
  echo '{"version":1,"hooks":{"session-end":true}}' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled_explicit hooks session-end
  [ "$status" -eq 0 ]
}

@test "toolu_enabled_explicit: disabled when explicitly false" {
  echo '{"version":1,"hooks":{"session-end":false}}' > "$HOME/.claude/toolu.config.json"
  run toolu_enabled_explicit hooks session-end
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: success when setup_done is true" {
  echo '{"version":1,"project":{"setup_done":true}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 0 ]
}

@test "toolu_flag_true: failure when key absent" {
  echo '{"version":1,"project":{"other":true}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: failure when value is false" {
  echo '{"version":1,"project":{"setup_done":false}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: failure when value is non-true string" {
  echo '{"version":1,"project":{"setup_done":"yes"}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: failure when value is a number" {
  echo '{"version":1,"project":{"setup_done":1}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: failure on malformed JSON" {
  printf '{' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true project setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: failure when jq unavailable" {
  # Stub dir holds only the commands the loader needs (no jq), so `command -v
  # jq` misses while bash/git/coreutils still resolve. env -i scopes the PATH
  # override to the child shell.
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash git cat printf mkdir; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c"
  done
  echo '{"version":1,"project":{"setup_done":true}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run env -i HOME="$HOME" CLAUDE_PROJECT_DIR="$CLAUDE_PROJECT_DIR" PATH="$STUB" bash -c \
    ". \"$REPO_ROOT/hooks/lib/config.sh\"; toolu_flag_true project setup_done"
  [ "$status" -eq 1 ]
}

@test "toolu_flag_true: success with the real comemory/setup_done key" {
  # Same category/name session-start.sh actually queries.
  echo '{"version":1,"comemory":{"setup_done":true}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_true comemory setup_done
  [ "$status" -eq 0 ]
}

@test "toolu_flag_false: success only when explicitly boolean false" {
  echo '{"version":1,"comemory":{"setup_done":false}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_false comemory setup_done
  [ "$status" -eq 0 ]
}

@test "toolu_flag_false: failure when value is true" {
  echo '{"version":1,"comemory":{"setup_done":true}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_false comemory setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_false: failure when key absent" {
  echo '{"version":1,"comemory":{"other":false}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_false comemory setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_false: failure when value is non-false string" {
  echo '{"version":1,"comemory":{"setup_done":"false"}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_false comemory setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_false: failure on malformed JSON" {
  printf '{' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run toolu_flag_false comemory setup_done
  [ "$status" -eq 1 ]
}

@test "toolu_flag_false: failure when jq unavailable" {
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash git cat printf mkdir; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c"
  done
  echo '{"version":1,"comemory":{"setup_done":false}}' > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"
  run env -i HOME="$HOME" CLAUDE_PROJECT_DIR="$CLAUDE_PROJECT_DIR" PATH="$STUB" bash -c \
    ". \"$REPO_ROOT/hooks/lib/config.sh\"; toolu_flag_false comemory setup_done"
  [ "$status" -eq 1 ]
}
