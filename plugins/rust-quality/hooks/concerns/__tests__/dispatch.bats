#!/usr/bin/env bats
# Covers the rust-quality 00-preamble (project/lib detection, file-path
# extraction across Write/Edit/MultiEdit) and 99-finalize (multi-slot gate
# write/clear) concerns. Drives the ASSEMBLED registry module — register.sh
# concatenates concerns/[0-9][0-9]-*.sh into one runtime script.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)

  # Assemble the registry module exactly as production does: point register.sh
  # at a temp CLAUDE_CONFIG_DIR and run it with no stdin.
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  HOOK="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/rust-quality@toolu__rust-quality.sh"

  # Real project root for fixtures (replaces the monolith suite's flat $TMP).
  TMP_PROJ="$TMP/proj"
  mkdir -p "$TMP_PROJ"
  cd "$TMP_PROJ"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Set up minimal Rust project so detect_rust passes.
_rust_project() {
  printf '[package]\nname = "x"\nversion = "0.1.0"\n' > Cargo.toml
  mkdir -p src
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "rust-quality: no-op outside a Rust project (no Cargo.toml)" {
  payload='{"tool_input":{"file_path":"/nonexistent.rs"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "rust-quality: Edit tool extracts file path and flags violations (regression)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
#[allow(dead_code)]
fn helper() {}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
}

# Regression: the PostToolUse matcher includes MultiEdit, but the file-path
# extraction only ran for Write/Edit — a MultiEdit on a .rs file (with
# CLAUDE_FILE_PATHS unset) silently skipped all quality checks.
@test "rust-quality: MultiEdit extracts file path and flags violations (regression)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
#[allow(dead_code)]
fn helper() {}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=MultiEdit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
}

@test "rust-quality: gate is cleared when the failing file is re-edited clean" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > src/bad.rs <<'EOF'
#[allow(dead_code)]
fn helper() {}
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/bad.rs"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"

  # Clean version: drop the #[allow] AND carry a //! header so rule_module_doc
  # (now part of the gate) is satisfied — otherwise the file is not truly clean.
  cat > src/bad.rs <<'EOF'
//! Helper module.
fn helper() {}
EOF
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -e '.source == "rust-quality-hook"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
}

@test "rust-quality: exits 0 silently when TOOLU_LIB_DIR is unset (fail soft)" {
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/main.rs"}}'
  run env -u TOOLU_LIB_DIR tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "rust-quality: clearing one file does not clobber another file's failure" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  printf '#[allow(dead_code)]\nfn a() {}\n' > src/a.rs
  printf '#[allow(dead_code)]\nfn b() {}\n' > src/b.rs
  payload_a='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/a.rs"}}'
  payload_b='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/b.rs"}}'
  GATE="$TMP_PROJ/.claude/tmp/quality-gate-status.json"

  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"

  # b.rs goes clean — a.rs's violation must survive and keep the gate failing.
  # (//! header keeps rule_module_doc satisfied so "clean" is truly clean.)
  printf '//! B module.\nfn b() {}\n' > src/b.rs
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e --arg f "$TMP_PROJ/src/a.rs" '.entries[$f]' "$GATE"

  # a.rs goes clean too — now the gate may pass. (//! header satisfies
  # rule_module_doc so the file is truly clean under the full rule set.)
  printf '//! A module.\nfn a() {}\n' > src/a.rs
  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE"
}

@test "rust-quality: failing gate owned by another hook survives a rust fail->clear cycle" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  mkdir -p "$TMP_PROJ/.claude/tmp"
  GATE="$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -n '{status: "failing", reason: "TS violation", source: "ts-quality-hook",
          file: "/p/x.ts", violations: "bad ts\n", updatedAt: "2026-01-01T00:00:00Z"}' > "$GATE"

  printf '#[allow(dead_code)]\nfn a() {}\n' > src/a.rs
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/src/a.rs"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "rust-quality-hook"' "$GATE"

  # Rust file goes clean — the TS failure must be promoted back, not erased.
  # (//! header satisfies rule_module_doc so the rust file is truly clean.)
  printf '//! A module.\nfn a() {}\n' > src/a.rs
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "ts-quality-hook"' "$GATE"
  jq -e '.reason == "TS violation"' "$GATE"
}
