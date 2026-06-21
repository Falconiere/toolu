#!/usr/bin/env bats
# Real end-to-end harness for the per-edit Rust gate AS ASSEMBLED by register.sh.
# register.sh concatenates 00-preamble + lib/rust-rules*.sh + the concern
# fragments into ONE module; this drives that exact module (not the fragments,
# not the rule_* functions in isolation) against REAL .rs fixtures — no mocks.
#
# Covers:
#   AC-15  nested-crate firing: a crate nested under a subdir with NO root
#          Cargo.toml still fires; a .rs with no enclosing Cargo.toml no-ops.
#   AC-7   parity: the (rule, line) block-tuples the assembled gate surfaces
#          EQUAL the tuples from calling the rule_* functions directly — proving
#          the thin wrapper drops/mangles nothing.

SPEC="rust-quality@toolu"
MODULE="${SPEC}__rust-quality.sh"

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"

  TMP=$(mktemp -d)
  HOOKS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  REGISTER="$HOOKS_DIR/register.sh"
  LIB_SRC_DIR="$HOOKS_DIR/lib"
  FIXTURES="$(cd "$(dirname "$BATS_TEST_FILENAME")/fixtures" && pwd)"
  # Core toolu lib (detect.sh, quality-config.sh) — provided via TOOLU_LIB_DIR in
  # production; the test provides it here.
  TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/lib" && pwd)"
  export TOOLU_LIB_DIR

  # Assemble the runtime module exactly as a SessionStart would.
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  bash "$REGISTER" </dev/null
  ASSEMBLED="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/$MODULE"
  [ -f "$ASSEMBLED" ]
}
teardown() { rm -rf "$TMP"; }

# Copy a committed fixture tree into a fresh, hermetic, NON-git TMP root and echo
# its path. Strips any stale .claude/ gate residue so each test starts clean.
_isolate_fixture() {
  local name="$1"
  local dst="$TMP/$name"   # separate stmt: `local a=$1 b=$TMP/$a` mis-orders in bash
  rm -rf "$dst"
  cp -R "$FIXTURES/$name" "$dst"
  rm -rf "$dst/.claude"
  printf '%s' "$dst"
}

# Run the assembled module on FILE under PROJECT_ROOT, simulating a Write/Edit.
# Echoes the module's stdout (the PostToolUse JSON, if any).
_run_gate() {
  local file="$1" root="$2"
  local payload
  payload=$(printf '{"tool_input":{"file_path":"%s"}}' "$file")
  env TOOLU_LIB_DIR="$TOOLU_LIB_DIR" PROJECT_ROOT="$root" \
      tool_name=Write input="$payload" \
      bash "$ASSEMBLED" </dev/null
}

# === AC-15: nested-crate firing =========================================

@test "AC-15: a violating .rs in a crate nested under a subdir (no root Cargo.toml) fires" {
  # Copy the nested fixture into a NON-git TMP tree so detect_rust (git-root
  # Cargo.toml) finds nothing — the ONLY way the gate can fire is via
  # nearest_cargo_toml walking up to the enclosing crate's Cargo.toml.
  root=$(_isolate_fixture nested)
  target="$root/packages/backend/crates/api/src/handler.rs"
  [ -f "$target" ]
  [ ! -f "$root/Cargo.toml" ]   # no root Cargo.toml — detect_rust would no-op

  run _run_gate "$target" "$root"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "QUALITY VIOLATION"
}

@test "AC-15: a .rs with NO enclosing Cargo.toml anywhere above it no-ops" {
  root=$(_isolate_fixture nested)
  target="$root/loose/orphan.rs"
  [ -f "$target" ]

  run _run_gate "$target" "$root"
  [ "$status" -eq 0 ]
  # No crate above it → the gate must produce no QUALITY VIOLATION at all.
  ! echo "$output" | grep -q "QUALITY VIOLATION"
}

# === AC-7: assembled-gate vs direct rule_* parity =======================

# The wrapper (15-rust-rules.sh) routes each block TSV row's MESSAGE field to
# add_error verbatim, so the set of messages the assembled gate records for a
# file is exactly the set of block-row messages the rule_* functions emit for
# it — IF nothing is dropped or mangled. The message field uniquely encodes the
# (rule, line) tuple (each carries the rule's identity and the offending line),
# so comparing the SETS of messages is a faithful (rule, line) parity proof.
# We compare the gate's recorded per-file violations against the rule lib's
# direct block output; any drop/merge/duplication makes the diff non-empty.

# All block-row messages from the rule_* functions for FILE, sorted.
_direct_block_messages() {
  local file="$1"
  (
    # shellcheck source=/dev/null
    . "$TOOLU_LIB_DIR/detect.sh"
    # shellcheck source=/dev/null
    . "$TOOLU_LIB_DIR/quality-config.sh"
    # shellcheck source=/dev/null
    . "$LIB_SRC_DIR/rust-rules.sh"
    local fn
    for fn in rule_file_size rule_mod_rs_no_logic rule_generic_name \
              rule_test_location rule_module_doc rule_fn_size \
              rule_impl_size rule_layering_file; do
      "$fn" "$file"
    done
  ) | awk -F '\t' 'NF >= 6 && $2 == "block" { print $6 }' | sort
}

# The block-row messages the assembled gate recorded for FILE, sorted. The
# multi-slot gate writer keys violations by file under .entries[<file>]; we read
# THAT file's slot (not the aggregated top-level .violations, which merges every
# file's failures) so the parity is strictly per-file. add_error joins messages
# with a literal "\n", so we expand those back to newlines.
_gate_block_messages() {
  local file="$1" root="$2"
  _run_gate "$file" "$root" >/dev/null
  local gate="$root/.claude/tmp/quality-gate-status.json"
  [ -f "$gate" ] || { printf ''; return 0; }
  jq -r --arg f "$file" '
    (.entries[$f].violations // .violations // "")
  ' "$gate" \
    | sed 's/\\n/\n/g' \
    | awk 'NF' | sort
}

@test "AC-7: assembled gate block messages EQUAL direct rule_* block messages (bare_pub.rs)" {
  # A single-crate fixture so the gate fires via detect-rust/nearest-crate; the
  # file trips the layering block rule (bare pub in a child src/ module).
  root=$(_isolate_fixture crate)
  target="$root/src/bare_pub.rs"

  direct=$(_direct_block_messages "$target")
  gated=$(_gate_block_messages "$target" "$root")
  [ -n "$gated" ]
  diff <(printf '%s\n' "$direct") <(printf '%s\n' "$gated")
}

@test "AC-7: parity holds for a file tripping several block rules (no_module_doc.rs)" {
  root=$(_isolate_fixture crate)
  target="$root/src/no_module_doc.rs"

  direct=$(_direct_block_messages "$target")
  gated=$(_gate_block_messages "$target" "$root")
  [ -n "$gated" ]
  diff <(printf '%s\n' "$direct") <(printf '%s\n' "$gated")
}

@test "AC-7: parity holds for the inline-cfg(test) lib.rs fixture (test-location block)" {
  root=$(_isolate_fixture crate)
  target="$root/src/lib.rs"

  direct=$(_direct_block_messages "$target")
  gated=$(_gate_block_messages "$target" "$root")
  [ -n "$gated" ]
  diff <(printf '%s\n' "$direct") <(printf '%s\n' "$gated")
}
