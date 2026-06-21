#!/usr/bin/env bats
# Unit tests for the canonical Rust rule library (lib/rust-rules.sh +
# lib/rust-rules-layering.sh). Each rule_* echoes TSV violation records:
#
#     <rule>\t<severity>\t<file>\t<line>\t<autofix>\t<message>
#
# Tests assert on the TSV fields (rule id + severity), NOT on message text.
# Fixtures are REAL .rs files under __tests__/fixtures/ (not strings-in-test).
# Maps to spec AC-1..AC-6 plus nearest_cargo_toml.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this env
# var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

FIXTURES="$(cd "$(dirname "$BATS_TEST_FILENAME")/fixtures" && pwd)"
CRATE="$FIXTURES/crate"

setup() {
  # The CALLER sources these libs; we mirror that here, then the rule lib.
  # shellcheck source=/dev/null
  . "$TOOLU_LIB_DIR/detect.sh"
  # shellcheck source=/dev/null
  . "$TOOLU_LIB_DIR/quality-config.sh"
  # shellcheck source=/dev/null
  . "$(cd "$(dirname "$BATS_TEST_FILENAME")/../lib" && pwd)/rust-rules.sh"
}

# --- helpers -------------------------------------------------------------

# Extract field N (1-based) from the TSV lines matching rule id $2 in $1.
# Usage: tsv_field <output> <rule> <field-index>
tsv_field() {
  printf '%s\n' "$1" | awk -F '\t' -v r="$2" -v n="$3" '$1 == r { print $n }'
}

# Count TSV records (rows) whose rule id equals $2 in $1.
tsv_rows() {
  printf '%s\n' "$1" | awk -F '\t' -v r="$2" 'NF >= 6 && $1 == r { c++ } END { print c+0 }'
}

# Count ALL non-empty TSV rows in $1.
tsv_total() {
  printf '%s\n' "$1" | awk -F '\t' 'NF >= 6 { c++ } END { print c+0 }'
}

# === AC-1: file-size =====================================================
@test "rule_file_size: a >250-code-line file is flagged block/split at line 1" {
  run rule_file_size "$CRATE/src/oversized.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" file-size)" -eq 1 ]
  [ "$(tsv_field "$output" file-size 2)" = "block" ]
  [ "$(tsv_field "$output" file-size 4)" = "1" ]
  [ "$(tsv_field "$output" file-size 5)" = "split" ]
}

@test "rule_file_size: a compact file produces no records" {
  run rule_file_size "$CRATE/src/compact.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === AC-2: mod-rs-no-logic ==============================================
@test "rule_mod_rs_no_logic: a mod.rs with a fn is flagged block/restructure" {
  run rule_mod_rs_no_logic "$CRATE/src/feature/mod.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" mod-rs-no-logic)" -eq 1 ]
  [ "$(tsv_field "$output" mod-rs-no-logic 2)" = "block" ]
  [ "$(tsv_field "$output" mod-rs-no-logic 5)" = "restructure" ]
}

@test "rule_mod_rs_no_logic: a mod.rs with only mod/pub use/doc is clean" {
  run rule_mod_rs_no_logic "$CRATE/src/clean_mod/mod.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === AC-3: generic-name =================================================
@test "rule_generic_name: utils.rs is flagged block/rename at line 1" {
  run rule_generic_name "$CRATE/src/utils.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" generic-name)" -eq 1 ]
  [ "$(tsv_field "$output" generic-name 2)" = "block" ]
  [ "$(tsv_field "$output" generic-name 4)" = "1" ]
  [ "$(tsv_field "$output" generic-name 5)" = "rename" ]
}

@test "rule_generic_name: a descriptive name (signing.rs) is clean" {
  run rule_generic_name "$CRATE/src/signing.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

@test "rule_generic_name: shared/common.rs is exempt" {
  run rule_generic_name "$CRATE/src/shared/common.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === AC-4: test-location ================================================
@test "rule_test_location: inline #[cfg(test)] in src/ is flagged block/move" {
  run rule_test_location "$CRATE/src/lib.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" test-location)" -eq 1 ]
  [ "$(tsv_field "$output" test-location 2)" = "block" ]
  [ "$(tsv_field "$output" test-location 5)" = "move" ]
}

@test "rule_test_location: a flat tests/foo_test.rs is clean" {
  run rule_test_location "$CRATE/tests/foo_test.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

@test "rule_test_location: a nested tests/integration/x_test.rs is flagged block/move" {
  run rule_test_location "$CRATE/tests/integration/x_test.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" test-location)" -eq 1 ]
  [ "$(tsv_field "$output" test-location 2)" = "block" ]
  [ "$(tsv_field "$output" test-location 5)" = "move" ]
}

# === AC-5: module-doc ===================================================
@test "rule_module_doc: no //! header is flagged block" {
  run rule_module_doc "$CRATE/src/no_module_doc.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" module-doc)" -eq 1 ]
  [ "$(tsv_field "$output" module-doc 2)" = "block" ]
}

@test "rule_module_doc: //! without sections is flagged advisory" {
  run rule_module_doc "$CRATE/src/partial_doc.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" module-doc)" -eq 1 ]
  [ "$(tsv_field "$output" module-doc 2)" = "advisory" ]
}

@test "rule_module_doc: //! with both sections is clean" {
  run rule_module_doc "$CRATE/src/full_doc.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === fn-size ============================================================
@test "rule_fn_size: a >80-line fn is flagged block/split" {
  run rule_fn_size "$CRATE/src/long_fn.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" fn-size)" -eq 1 ]
  [ "$(tsv_field "$output" fn-size 2)" = "block" ]
  [ "$(tsv_field "$output" fn-size 5)" = "split" ]
}

@test "rule_fn_size: a short fn is clean" {
  run rule_fn_size "$CRATE/src/short_fn.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === impl-size ==========================================================
@test "rule_impl_size: a >200-line impl is flagged block/split" {
  run rule_impl_size "$CRATE/src/big_impl.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" impl-size)" -eq 1 ]
  [ "$(tsv_field "$output" impl-size 2)" = "block" ]
  [ "$(tsv_field "$output" impl-size 5)" = "split" ]
}

@test "rule_impl_size: a small impl is clean" {
  run rule_impl_size "$CRATE/src/small_impl.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === AC-6: layering =====================================================
@test "rule_layering_file: a bare pub child item is flagged block/manual" {
  run rule_layering_file "$CRATE/src/bare_pub.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" layering)" -ge 1 ]
  [ "$(tsv_field "$output" layering 2)" = "block" ]
  [ "$(tsv_field "$output" layering 5)" = "manual" ]
}

@test "rule_layering_file: a pub(super) child item is clean" {
  run rule_layering_file "$CRATE/src/scoped_pub.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

# === nearest_cargo_toml =================================================
@test "nearest_cargo_toml: finds an ancestor Cargo.toml" {
  run nearest_cargo_toml "$CRATE/src/feature/mod.rs"
  [ "$status" -eq 0 ]
  [ "$output" = "$CRATE/Cargo.toml" ]
}

@test "nearest_cargo_toml: empty when no ancestor Cargo.toml exists" {
  local sandbox
  sandbox="$(mktemp -d)"
  mkdir -p "$sandbox/a/b"
  printf '//! orphan\n' > "$sandbox/a/b/orphan.rs"
  run nearest_cargo_toml "$sandbox/a/b/orphan.rs"
  rm -rf "$sandbox"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# === config resolvers ===================================================
@test "rr_config_bool: modRsNoLogic=false disables rule_mod_rs_no_logic" {
  export TOOLU_CFG_JSON='{"lang":{"rust":{"modRsNoLogic":false}}}'
  export TOOLU_CFG_LOADED=1 _TOOLU_HAS_JQ=1
  run rule_mod_rs_no_logic "$CRATE/src/feature/mod.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}

@test "rr_config_list: forbiddenFileNames override flags signing.rs" {
  export TOOLU_CFG_JSON='{"lang":{"rust":{"forbiddenFileNames":["signing"]}}}'
  export TOOLU_CFG_LOADED=1 _TOOLU_HAS_JQ=1
  run rule_generic_name "$CRATE/src/signing.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_rows "$output" generic-name)" -eq 1 ]
}

@test "quality_threshold override: maxFileLines=400 lets a 300-line file pass" {
  export TOOLU_CFG_JSON='{"lang":{"rust":{"maxFileLines":400}}}'
  export TOOLU_CFG_LOADED=1 _TOOLU_HAS_JQ=1
  run rule_file_size "$CRATE/src/oversized.rs"
  [ "$status" -eq 0 ]
  [ "$(tsv_total "$output")" -eq 0 ]
}
