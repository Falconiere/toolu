#!/usr/bin/env bats
# Tests for quality-config.sh — the threshold resolver. Real jq, real files,
# no mocks.

setup() {
  TMP=$(mktemp -d)
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$TMP/project"
  mkdir -p "$HOME/.claude" "$CLAUDE_PROJECT_DIR/.claude"
  ( cd "$CLAUDE_PROJECT_DIR" \
      && git init -q \
      && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init )
  cd "$CLAUDE_PROJECT_DIR"
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Source the libs in-process and reset the loader state per test.
load_libs() {
  TOOLU_LIB_DIR="$(cd "${BATS_TEST_DIRNAME}/.." && pwd)"
  export TOOLU_LIB_DIR
  # shellcheck disable=SC1091
  . "$TOOLU_LIB_DIR/quality-config.sh"
  TOOLU_CFG_JSON='{}'
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  _QC_PROJECT_ROOT=""
  _QC_PROJECT_ROOT_CACHED=0
}

_project_cfg() { printf '%s' "$1" > "$CLAUDE_PROJECT_DIR/.claude/toolu.config.json"; }
_user_cfg()    { printf '%s' "$1" > "$HOME/.claude/toolu.config.json"; }

@test "default when no config: TS 300, Rust 500/50/200" {
  load_libs
  run ts_max_file_lines;   [ "$output" = "300" ]
  run rust_max_file_lines; [ "$output" = "500" ]
  run rust_max_fn_lines;   [ "$output" = "50" ]
  run rust_max_impl_lines; [ "$output" = "200" ]
}

@test "default when no config: Python 400/50" {
  load_libs
  run python_max_file_lines; [ "$output" = "400" ]
  run python_max_fn_lines;   [ "$output" = "50" ]
}

@test "python project override wins over default" {
  _project_cfg '{"lang":{"python":{"maxFileLines":250,"maxFnLines":30}}}'
  load_libs
  run python_max_file_lines; [ "$output" = "250" ]
  run python_max_fn_lines;   [ "$output" = "30" ]
}

@test "python env-default override changes the built-in default" {
  export DEFAULT_PYTHON_MAX_FILE_LINES=111
  load_libs
  run python_max_file_lines
  [ "$output" = "111" ]
}

@test "quality_flag python noMocks: defaults to true when unset" {
  load_libs
  run quality_flag python noMocks true
  [ "$output" = "true" ]
}

@test "quality_flag python noMocks: false honored from project config" {
  _project_cfg '{"lang":{"python":{"noMocks":false}}}'
  load_libs
  run quality_flag python noMocks true
  [ "$output" = "false" ]
}

@test "project override wins over default" {
  _project_cfg '{"lang":{"ts":{"maxFileLines":120}}}'
  load_libs
  run ts_max_file_lines
  [ "$output" = "120" ]
}

@test "project override wins over eslint native config" {
  _project_cfg '{"lang":{"ts":{"maxFileLines":120}}}'
  printf '%s' '{"rules":{"max-lines":["error",250]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "120" ]
}

@test "eslint max-lines as bare number" {
  printf '%s' '{"rules":{"max-lines":250}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "250" ]
}

@test "eslint max-lines as [error, N]" {
  printf '%s' '{"rules":{"max-lines":["error",240]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "240" ]
}

@test "eslint max-lines as [error, {max: N}]" {
  printf '%s' '{"rules":{"max-lines":["error",{"max":222,"skipBlankLines":true}]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "222" ]
}

@test "eslint max-lines off falls through to default" {
  printf '%s' '{"rules":{"max-lines":"off"}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "300" ]
}

@test "eslint max-lines [off, N] severity falls through to default" {
  printf '%s' '{"rules":{"max-lines":["off",250]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "300" ]
}

@test "eslint max-lines [0, N] severity falls through to default" {
  printf '%s' '{"rules":{"max-lines":[0,250]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "300" ]
}

@test "oxlint config used when no eslint config" {
  printf '%s' '{"rules":{"max-lines":["error",333]}}' > "$CLAUDE_PROJECT_DIR/.oxlintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "333" ]
}

@test "both configs present: the active linter (detect_ts_linter=oxc) wins" {
  # .oxlintrc.json makes detect_ts_linter report oxc, so the gate reads the
  # oxlint limit — not whichever config file happens to be listed first.
  printf '%s' '{"rules":{"max-lines":250}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  printf '%s' '{"rules":{"max-lines":333}}' > "$CLAUDE_PROJECT_DIR/.oxlintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "333" ]
}

@test "biome active: native max-lines is not parsed, falls through to default" {
  # biome outranks eslint in detect_ts_linter and has no machine-readable
  # max-lines rule here, so an .eslintrc.json limit must NOT be picked up.
  printf '%s' '{}' > "$CLAUDE_PROJECT_DIR/biome.json"
  printf '%s' '{"rules":{"max-lines":250}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "300" ]
}

@test "flat eslint config (JS) is skipped gracefully -> default" {
  printf '%s' 'export default [{ rules: { "max-lines": 200 } }]' > "$CLAUDE_PROJECT_DIR/eslint.config.mjs"
  load_libs
  run ts_max_file_lines
  [ "$status" -eq 0 ]
  [ "$output" = "300" ]
}

@test "malformed .eslintrc.json falls through to default, no error" {
  printf '%s' '{ broken' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$status" -eq 0 ]
  [ "$output" = "300" ]
}

@test "rust fn/impl never consult TS native config" {
  printf '%s' '{"rules":{"max-lines":["error",111]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run rust_max_fn_lines;   [ "$output" = "50" ]
  run rust_max_file_lines; [ "$output" = "500" ]
}

@test "user vs project merge: project wins" {
  _user_cfg    '{"lang":{"ts":{"maxFileLines":111}}}'
  _project_cfg '{"lang":{"ts":{"maxFileLines":222}}}'
  load_libs
  run ts_max_file_lines
  [ "$output" = "222" ]
}

@test "zero / negative thresholds are rejected -> default" {
  _project_cfg '{"lang":{"ts":{"maxFileLines":0}}}'
  printf '%s' '{"rules":{"max-lines":["error",-5]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "300" ]
}

@test "rust project override works" {
  _project_cfg '{"lang":{"rust":{"maxFileLines":250,"maxFnLines":30,"maxImplLines":150}}}'
  load_libs
  run rust_max_file_lines; [ "$output" = "250" ]
  run rust_max_fn_lines;   [ "$output" = "30" ]
  run rust_max_impl_lines; [ "$output" = "150" ]
}

@test "project override accepts a stringified positive int" {
  _project_cfg '{"lang":{"ts":{"maxFileLines":"120"}}}'
  load_libs
  run ts_max_file_lines
  [ "$output" = "120" ]
}

@test "eslint max-lines accepts a stringified number in the array" {
  printf '%s' '{"rules":{"max-lines":["error","250"]}}' > "$CLAUDE_PROJECT_DIR/.eslintrc.json"
  load_libs
  run ts_max_file_lines
  [ "$output" = "250" ]
}

@test "ts_max_file_lines_resolved returns value and source together" {
  _project_cfg '{"lang":{"ts":{"maxFileLines":120}}}'
  load_libs
  run ts_max_file_lines_resolved
  [ "$output" = "120 override" ]
}

@test "quality_flag: true honored from project config" {
  _project_cfg '{"lang":{"ts":{"noMocks":true}}}'
  load_libs
  run quality_flag ts noMocks false
  [ "$output" = "true" ]
}

@test "quality_flag: false honored from project config" {
  _project_cfg '{"lang":{"rust":{"noMocks":false}}}'
  load_libs
  run quality_flag rust noMocks true
  [ "$output" = "false" ]
}

@test "quality_flag: missing key falls back to default" {
  load_libs
  run quality_flag ts noMocks true
  [ "$output" = "true" ]
}

@test "quality_flag: non-boolean junk falls back to default" {
  _project_cfg '{"lang":{"ts":{"noMocks":"nope"}}}'
  load_libs
  run quality_flag ts noMocks true
  [ "$output" = "true" ]
}
