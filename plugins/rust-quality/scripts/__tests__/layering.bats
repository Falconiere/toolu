#!/usr/bin/env bats
# Tests for the cross-file, repo-wide layering heuristic rule_layering_crate
# (spec AC-17, Open Question #1) — driven against REAL Rust crate fixtures, no
# mocks. rule_layering_crate is the best-effort ast-grep import-graph check: it
# flags a sibling module reaching past another sibling's mod.rs re-export surface
# into its private internals. It is ADVISORY (never block) so AC-17 stays
# non-blocking, and conservative (uncertain -> never flag) so it cannot false-
# positive on clean re-export usage.
#
# Assertions cover ONLY the documented best-effort behavior:
#   * a reach into a non-reexported internal item -> one advisory layering row;
#   * usage of only the re-exported public item   -> NO layering row;
#   * the row's severity is `advisory` (proving the rule is non-blocking);
#   * via the scanner, the advisory rides into JSON violations[] AND the scan
#     exit code stays 0 (advisory does not block).
# Glob/macro/rename precision is a documented limit and is NOT asserted here.

LOCAL_FIX="$(cd "$(dirname "$BATS_TEST_FILENAME")/fixtures" && pwd)"
SCRIPTS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
SCAN="$SCRIPTS_DIR/rust-scan.sh"
# Core toolu lib + the canonical rule library (which sources rust-rules-layering.sh).
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR
RULES_LIB="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../hooks/lib" && pwd)/rust-rules.sh"

DIRTY_CRATE="$LOCAL_FIX/layering_dirty"   # b reaches a's non-reexported internal_thing
CLEAN_CRATE="$LOCAL_FIX/layering_clean"   # b uses only a's re-exported PublicThing

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
}

# Call rule_layering_crate directly on a crate root; echoes its raw TSV rows.
_layering_rows() {
  local root="$1"
  bash -c '
    export TOOLU_LIB_DIR="'"$TOOLU_LIB_DIR"'"
    . "$TOOLU_LIB_DIR/detect.sh"
    . "$TOOLU_LIB_DIR/quality-config.sh"
    . "'"$RULES_LIB"'"
    rule_layering_crate "'"$root"'"
  '
}

# === Cross-file reach into a sibling's internals -> advisory row ==========

@test "AC-17: a sibling reaching a non-reexported internal emits ONE advisory layering row" {
  local rows
  rows=$(_layering_rows "$DIRTY_CRATE")
  # Exactly one layering row, at the offending `use crate::a::internal_thing;`.
  [ "$(printf '%s\n' "$rows" | awk -F '\t' '$1 == "layering" { c++ } END { print c+0 }')" -eq 1 ]
  # Fields: rule=layering, severity=advisory, file=b/worker.rs, autofix=manual.
  local rule sev file line autofix
  IFS=$'\t' read -r rule sev file _ autofix _ <<< "$(printf '%s\n' "$rows" | awk -F '\t' '$1 == "layering"')"
  [ "$rule" = "layering" ]
  [ "$sev" = "advisory" ]
  [ "$autofix" = "manual" ]
  [[ "$file" == */src/b/worker.rs ]]
}

@test "AC-17: the offending row names the private item and the target module" {
  local row
  row=$(_layering_rows "$DIRTY_CRATE" | awk -F '\t' '$1 == "layering"')
  [[ "$row" == *"internal_thing"* ]]
  [[ "$row" == *"'a'"* ]]
}

# === Clean re-export usage -> NO false positive ==========================

@test "AC-17: using only the re-exported public item emits NO layering row (no false positive)" {
  local rows
  rows=$(_layering_rows "$CLEAN_CRATE")
  [ "$(printf '%s\n' "$rows" | awk -F '\t' '$1 == "layering" { c++ } END { print c+0 }')" -eq 0 ]
}

# === Non-blocking: severity advisory + scanner exit 0 ====================

@test "AC-17: severity is advisory (the rule is non-blocking)" {
  local sev
  sev=$(_layering_rows "$DIRTY_CRATE" | awk -F '\t' '$1 == "layering" { print $2; exit }')
  [ "$sev" = "advisory" ]
}

@test "AC-17: via the scanner the advisory rides into JSON and exit code stays 0" {
  run bash "$SCAN" --path "$DIRTY_CRATE" --json
  [ "$status" -eq 0 ]   # advisory must NOT change the exit code
  local hit
  hit=$(printf '%s' "$output" \
    | jq -c '[.crates[].violations[]
             | select(.rule=="layering" and .severity=="advisory")
             | {file, severity}]')
  [ "$hit" = '[{"file":"src/b/worker.rs","severity":"advisory"}]' ]
}

@test "AC-17: the clean crate yields no advisory layering row in the scan JSON" {
  run bash "$SCAN" --path "$CLEAN_CRATE" --json
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" \
      | jq -r '[.crates[].violations[]
               | select(.rule=="layering" and .severity=="advisory")] | length')" -eq 0 ]
}
