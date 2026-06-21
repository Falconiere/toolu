#!/usr/bin/env bats
# Tests for the repo-wide scanner scripts/rust-scan.sh — driven against REAL
# Rust crate fixtures (no mocks). The scanner SOURCES the canonical rule library
# (hooks/lib/rust-rules.sh), so these tests double as a contract check that the
# walk/report layer preserves every rule_* record verbatim.
#
# Coverage:
#   AC-1   a real >250-code-line .rs reports a file-size violation (file+line)
#          in --json; a clean crate yields summary.total == 0.
#   AC-13  --path <nested> resolves a workspace whose Cargo.toml is NOT at the
#          scanned dir (nested below it) and scans its members.
#   AC-7   for one fixture .rs, the (rule,line) set in the scan JSON equals
#          calling the rule_* functions directly — the scan drops/mangles nothing.
#   non-Rust target -> non-zero exit + a clear message (never empty JSON).
#   --json is valid JSON.
#   --staged limits to staged .rs files.

SCRIPTS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
SCAN="$SCRIPTS_DIR/rust-scan.sh"
HOOKS_FIX="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../hooks/__tests__/fixtures" && pwd)"
LOCAL_FIX="$(cd "$(dirname "$BATS_TEST_FILENAME")/fixtures" && pwd)"
# Core toolu lib — provided via TOOLU_LIB_DIR in production; the test provides it.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR
RULES_LIB="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../hooks/lib" && pwd)/rust-rules.sh"

DIRTY_CRATE="$HOOKS_FIX/crate"        # the rich, intentionally-violating fixture
CLEAN_CRATE="$LOCAL_FIX/clean"        # a structurally clean crate (zero violations)
NESTED_DIR="$HOOKS_FIX/nested/packages/backend"  # crate lives BELOW this dir

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
}

# === AC-1: file-size present + clean crate clean ========================

@test "AC-1: a >250-code-line .rs reports a file-size violation with file+line in --json" {
  run bash "$SCAN" --path "$DIRTY_CRATE" --json
  [ "$status" -eq 0 ]
  # Exactly the oversized fixture is flagged file-size at line 1.
  local hit
  hit=$(printf '%s' "$output" \
    | jq -c '[.crates[].violations[]
             | select(.rule=="file-size")
             | {file, line, severity}]')
  [ "$hit" = '[{"file":"src/oversized.rs","line":1,"severity":"block"}]' ]
}

@test "AC-1: a structurally clean crate yields summary.total == 0" {
  run bash "$SCAN" --path "$CLEAN_CRATE" --json
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | jq -r '.summary.total')" -eq 0 ]
  [ "$(printf '%s' "$output" | jq -r '[.crates[].violations[]] | length')" -eq 0 ]
}

# === AC-13: nested workspace resolution =================================

@test "AC-13: --path <nested> resolves a workspace whose Cargo.toml is below the scanned dir" {
  # The scanned dir has NO Cargo.toml; the crate sits at crates/api/ below it.
  [ ! -f "$NESTED_DIR/Cargo.toml" ]
  run bash "$SCAN" --path "$NESTED_DIR" --json
  [ "$status" -eq 0 ]
  # The member 'api' is discovered and its handler.rs violations returned.
  [ "$(printf '%s' "$output" | jq -r '.crates | length')" -eq 1 ]
  [ "$(printf '%s' "$output" | jq -r '.crates[0].name')" = "api" ]
  [ "$(printf '%s' "$output" | jq -r '[.crates[0].violations[] | .file] | unique | .[0]')" = "src/handler.rs" ]
  [ "$(printf '%s' "$output" | jq -r '.summary.total')" -gt 0 ]
}

# === AC-7: scan output == direct rule_* output (define-once parity) =====

# All (rule:line) tuples from calling every rule_* directly on FILE, sorted.
_direct_tuples() {
  local file="$1"
  bash -c '
    export TOOLU_LIB_DIR="'"$TOOLU_LIB_DIR"'"
    . "$TOOLU_LIB_DIR/detect.sh"
    . "$TOOLU_LIB_DIR/quality-config.sh"
    . "'"$RULES_LIB"'"
    for fn in rule_file_size rule_mod_rs_no_logic rule_generic_name \
              rule_test_location rule_module_doc rule_fn_size \
              rule_impl_size rule_layering_file; do
      "$fn" "'"$file"'"
    done
  ' | awk -F '\t' 'NF >= 6 { print $1 ":" $4 }' | sort
}

@test "AC-7: scan (rule,line) tuples EQUAL direct rule_* tuples for a multi-violation file" {
  local rel="src/big_impl.rs"   # trips impl-size AND layering at distinct lines
  local direct scan
  direct=$(_direct_tuples "$DIRTY_CRATE/$rel")
  [ -n "$direct" ]   # the chosen fixture must actually trip rules
  scan=$(bash "$SCAN" --path "$DIRTY_CRATE" --json \
    | jq -r --arg f "$rel" '.crates[].violations[]
        | select(.file==$f) | "\(.rule):\(.line)"' | sort)
  diff <(printf '%s\n' "$direct") <(printf '%s\n' "$scan")
}

# === non-Rust target: non-zero + message, never empty JSON ==============

@test "non-Rust target exits non-zero with a clear message and no empty JSON" {
  local notrust
  notrust=$(mktemp -d)
  printf 'hello\n' > "$notrust/README.md"
  run bash "$SCAN" --path "$notrust" --json
  rm -rf "$notrust"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not a Cargo workspace"* ]]
  # Must NOT have produced a JSON object an audit could read as clean.
  ! printf '%s' "$output" | jq -e . >/dev/null 2>&1
}

# === --json is valid JSON ================================================

@test "--json emits valid JSON (pipes through jq cleanly)" {
  run bash "$SCAN" --path "$DIRTY_CRATE" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | jq -e 'has("repo") and has("crates") and has("config") and has("summary")' >/dev/null
}

# === --staged limits to staged .rs files ================================

@test "--staged scans only staged .rs files (pre-commit parity)" {
  command -v git >/dev/null 2>&1 || skip "git not installed"
  local repo
  repo=$(mktemp -d)
  cp -R "$DIRTY_CRATE/." "$repo/"
  (
    cd "$repo"
    git init -q
    git config user.email t@t.io
    git config user.name t
    git add -A
    git commit -qm init
    # Stage exactly one violating .rs (utils.rs trips generic-name). The other
    # known offenders (oversized.rs file-size, etc.) stay committed/unstaged and
    # MUST NOT appear in a --staged scan.
    printf '\npub const EXTRA: u32 = 1;\n' >> src/utils.rs
    git add src/utils.rs
  )
  run bash "$SCAN" --path "$repo" --json --staged
  rm -rf "$repo"
  [ "$status" -eq 0 ]
  # The staged file's violation is present...
  [ "$(printf '%s' "$output" | jq -r '[.crates[].violations[] | select(.rule=="generic-name")] | length')" -eq 1 ]
  # ...and an UNSTAGED offender (oversized.rs file-size) is absent.
  [ "$(printf '%s' "$output" | jq -r '[.crates[].violations[] | select(.rule=="file-size")] | length')" -eq 0 ]
  # Every reported violation belongs to the single staged file.
  [ "$(printf '%s' "$output" | jq -r '[.crates[].violations[] | select(.file != "src/utils.rs")] | length')" -eq 0 ]
}
