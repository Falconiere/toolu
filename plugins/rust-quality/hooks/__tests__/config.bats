#!/usr/bin/env bats
# Config-driven thresholds + structural-rule toggles for the Rust rule lib.
# Step: config-keys. Real data only — AC-12 uses a real git repo + real
# .claude/toolu.config.json, no mocks.

setup() {
  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../../../.." && pwd)"
  export TOOLU_LIB_DIR="$REPO_ROOT/plugins/toolu/hooks/lib"
  # shellcheck source=/dev/null
  source "$TOOLU_LIB_DIR/detect.sh"
  # shellcheck source=/dev/null
  source "$TOOLU_LIB_DIR/quality-config.sh"
  # shellcheck source=/dev/null
  source "$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh"
}

# Build a Rust file with exactly N code lines (one leading //! doc comment,
# which count_code_lines excludes) under <dir>/src/big.rs.
_make_nline_rs() {
  local dir="$1" n="$2" i
  mkdir -p "$dir/src"
  {
    echo '//! fixture module'
    for ((i = 1; i <= n; i++)); do echo "pub const X${i}: u32 = ${i};"; done
  } > "$dir/src/big.rs"
}

@test "default rust file limit is 250 (was 500)" {
  run rust_max_file_lines
  [ "$status" -eq 0 ]
  [ "$output" = "250" ]
}

@test "default rust fn limit is 80 (was 50)" {
  run rust_max_fn_lines
  [ "$output" = "80" ]
}

@test "default rust impl limit is 200 (unchanged)" {
  run rust_max_impl_lines
  [ "$output" = "200" ]
}

@test "AC-12: project config override raises maxFileLines so a 300-line file passes" {
  tmp="$(mktemp -d)"
  git -C "$tmp" init -q
  mkdir -p "$tmp/.claude"
  printf '{"version":1,"lang":{"rust":{"maxFileLines":400}}}' > "$tmp/.claude/toolu.config.json"
  _make_nline_rs "$tmp" 300
  run bash -c "cd '$tmp' && unset TOOLU_CFG_LOADED TOOLU_CFG_JSON; source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; rule_file_size '$tmp/src/big.rs'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  rm -rf "$tmp"
}

@test "without override a 300-line file is flagged at the default 250" {
  tmp="$(mktemp -d)"
  git -C "$tmp" init -q
  _make_nline_rs "$tmp" 300
  run bash -c "cd '$tmp' && unset TOOLU_CFG_LOADED TOOLU_CFG_JSON; source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; rule_file_size '$tmp/src/big.rs'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"file-size"* ]]
  [[ "$output" == *"block"* ]]
  rm -rf "$tmp"
}

@test "rr_config_bool defaults true (1) for modRsNoLogic" {
  run bash -c "unset TOOLU_CFG_JSON; TOOLU_CFG_LOADED=1; source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; rr_config_bool modRsNoLogic 1"
  [ "$output" = "1" ]
}

@test "rr_config_bool honors an override to false (0)" {
  run bash -c "source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; export TOOLU_CFG_LOADED=1 TOOLU_CFG_JSON='{\"lang\":{\"rust\":{\"modRsNoLogic\":false}}}'; rr_config_bool modRsNoLogic 1"
  [ "$output" = "0" ]
}

@test "rr_config_list defaults to the forbidden-name csv" {
  run bash -c "unset TOOLU_CFG_JSON; TOOLU_CFG_LOADED=1; source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; rr_config_list forbiddenFileNames 'utils,helpers,common,misc'"
  [ "$output" = "utils,helpers,common,misc" ]
}

@test "rr_config_list honors an override array" {
  run bash -c "source '$TOOLU_LIB_DIR/detect.sh'; source '$TOOLU_LIB_DIR/quality-config.sh'; source '$REPO_ROOT/plugins/rust-quality/hooks/lib/rust-rules.sh'; export TOOLU_CFG_LOADED=1 TOOLU_CFG_JSON='{\"lang\":{\"rust\":{\"forbiddenFileNames\":[\"junk\",\"tmp\"]}}}'; rr_config_list forbiddenFileNames 'utils,helpers,common,misc'"
  [ "$output" = "junk,tmp" ]
}
