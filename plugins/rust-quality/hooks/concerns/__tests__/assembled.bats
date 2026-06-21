#!/usr/bin/env bats
# Equivalence harness for the rust-quality plugin's ASSEMBLED registry module.
#
# register.sh concatenates concerns/[0-9][0-9]-*.sh into one runtime module. That
# module must produce the SAME quality-gate-status.json as the original monolith
# (plugins/lang-quality/hooks/post-tools.d/rust-quality.sh). These tests drive
# BOTH on real git-init'd Rust projects and diff the resulting gate JSON, so a
# fragment reorder / clobber / dropped-suppression regression fails here.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

# The pre-split monolith — the behavioral oracle.
# Built from the existing plugins/ dir so it resolves even after lang-quality is
# deleted (the file then simply does not exist → the oracle tests skip).
MONOLITH="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)/lang-quality/hooks/post-tools.d/rust-quality.sh"

setup() {
  TMP=$(mktemp -d)

  # Assemble the registry module exactly as production does: point register.sh
  # at a temp CLAUDE_CONFIG_DIR and run it with no stdin.
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  ASSEMBLED="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/rust-quality@toolu__rust-quality.sh"

  # Real project root for fixtures.
  PROJ="$TMP/proj"
  mkdir -p "$PROJ"
  cd "$PROJ"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Minimal Rust project so detect_rust passes (verbatim from the monolith bats).
_rust_project() {
  printf '[package]\nname = "x"\nversion = "0.1.0"\n' > "$PROJ/Cargo.toml"
  mkdir -p "$PROJ/src"
  ( cd "$PROJ" && git add -A && git -c user.email=t@t -c user.name=t commit -q -m setup )
}

# Run a given hook script on a fixture file; print the resulting gate JSON.
# The gate is written to ONE of two branch paths — a failing run lands under
# $PROJECT_ROOT/.claude/tmp (the GATE_DIR the finalize fragment mkdir's), a
# passing run targets the same path. Print whichever exists.
_run_gate() {
  local hook="$1" file="$2"
  rm -rf "$PROJ/.claude/tmp"
  local payload='{"tool_input":{"file_path":"'"$file"'"}}'
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Write input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$hook" >/dev/null 2>&1
  local gate="$PROJ/.claude/tmp/quality-gate-status.json"
  if [ -f "$gate" ]; then cat "$gate"; fi
}

# Gate JSON with every updatedAt stripped — the only field that legitimately
# differs run-to-run. Everything else (status, source, reason, file, ordered
# violations, entries) must match between monolith and assembled.
_gate_canon() {
  _run_gate "$1" "$2" | jq -S 'del(.updatedAt) | (.entries // {}) |= with_entries(.value |= del(.updatedAt))'
}

@test "assembled module exists after register.sh" {
  [ -f "$ASSEMBLED" ]
}

# --- Deliverable A.2: oracle diff on a 2-violation fixture ---
# Catches fragment reorder/clobber: a file over the line limit AND containing
# .unwrap(). The gate must list both violations in the SAME order from both.
@test "rust assembled == monolith on a 2-violation fixture (oracle diff)" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  [ -f "$MONOLITH" ] || skip "monolith removed post-split; equivalence proven pre-delete"
  _rust_project
  mkdir -p "$PROJ/.claude"
  echo '{"lang":{"rust":{"maxFileLines":5}}}' > "$PROJ/.claude/toolu.config.json"
  {
    echo 'fn main() {'
    echo '    let x = thing().unwrap();'
    for i in $(seq 1 10); do echo "    let v$i = $i;"; done
    echo '}'
  } > "$PROJ/src/bad.rs"

  mono=$(_gate_canon "$MONOLITH" "$PROJ/src/bad.rs")
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.rs")

  # Both must actually be failing with both violations present (guard the oracle).
  echo "$mono" | jq -e '.status == "failing"'
  echo "$mono" | jq -e '.violations | contains("exceeds 5-line limit")'
  echo "$mono" | jq -e '.violations | contains(".unwrap()")'
  # Ordered: file-size rule (10-size-file) fires before the .unwrap rule (60).
  # Compare byte offsets of each marker within the single violations string
  # (newline-encoding agnostic): the prefix before "exceeds" is shorter than
  # the prefix before ".unwrap()".
  v=$(echo "$mono" | jq -r '.violations')
  before_size="${v%%exceeds 5-line limit*}"
  before_unwrap="${v%%.unwrap()*}"
  [ "${#before_size}" -lt "${#before_unwrap}" ]

  # The crux: identical gate JSON (source tag, reason, ordered violations, entries).
  [ "$mono" = "$asm" ]
}

# --- Deliverable A.3: no clobber — fix ONE of two violations ---
@test "rust assembled keeps gate failing after fixing one of two violations" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _rust_project
  mkdir -p "$PROJ/.claude"
  echo '{"lang":{"rust":{"maxFileLines":5}}}' > "$PROJ/.claude/toolu.config.json"
  {
    echo 'fn main() {'
    echo '    let x = thing().unwrap();'
    for i in $(seq 1 10); do echo "    let v$i = $i;"; done
    echo '}'
  } > "$PROJ/src/bad.rs"
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.rs")
  echo "$asm" | jq -e '.status == "failing"'

  # Fix ONLY the .unwrap() (drop the offending line) — the file is still over
  # the 5-line limit, so the gate must remain failing on the size violation.
  {
    echo 'fn main() {'
    for i in $(seq 1 10); do echo "    let v$i = $i;"; done
    echo '}'
  } > "$PROJ/src/bad.rs"
  still=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.rs")
  echo "$still" | jq -e '.status == "failing"'
  echo "$still" | jq -e '.violations | contains("exceeds 5-line limit")'
  echo "$still" | jq -e '.violations | (contains(".unwrap()") | not)'
}

# --- Deliverable A.4: suppression preserved ---
# Inline #[cfg(test)] mod tests must NOT raise a test-PLACEMENT violation in the
# assembled module (the suppression rule survives assembly) — and it matches the
# monolith.
@test "rust assembled: inline #[cfg(test)] mod produces no test-placement violation" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  cat > "$PROJ/src/lib.rs" <<'EOF'
pub fn add(a: u8, b: u8) -> u8 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/lib.rs")
  # No "test file outside tests/" placement violation from the assembled module.
  echo "$asm" | jq -e '((.violations // "") | contains("test file outside tests/")) | not'
  # And the assembled gate equals the monolith gate (while the oracle still exists).
  if [ -f "$MONOLITH" ]; then
    [ "$(_gate_canon "$MONOLITH" "$PROJ/src/lib.rs")" = "$asm" ]
  fi
}

# --- Deliverable A.5: clean file clears the gate ---
@test "rust assembled: clean file writes no failure (gate passing) and == monolith" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  # Truly clean under the FULL rule set: //! header (rule_module_doc), /// on the
  # item, and pub(crate) not bare pub (rule_layering_file flags a bare `pub` in a
  # child src/ module).
  cat > "$PROJ/src/good.rs" <<'EOF'
//! Reads a file.
/// Reads a file and returns its contents.
pub(crate) fn read_it() -> Result<String, std::io::Error> {
    let s = std::fs::read_to_string("x")?;
    Ok(s)
}
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/good.rs")
  # A clean file on a project with no prior gate writes NO failure: gate_clear_file
  # returns early (no entry to clear), so no gate file is produced. The assembled
  # module must do exactly the same — never leave a failing gate, and match the
  # monolith byte-for-byte (here: both produce nothing).
  [ -z "$asm" ] || echo "$asm" | jq -e '.status != "failing"'
  if [ -f "$MONOLITH" ]; then
    [ "$(_gate_canon "$MONOLITH" "$PROJ/src/good.rs")" = "$asm" ]
  fi
}

# Stronger criterion-5: the assembled module's CLEAR path actively flips a prior
# failure (recorded by the same hook+file) to passing — not just "writes nothing".
@test "rust assembled: re-editing a failing file clean flips the gate to passing" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_project
  GATE="$PROJ/.claude/tmp/quality-gate-status.json"
  payload='{"tool_input":{"file_path":"'"$PROJ"'/src/x.rs"}}'

  # Phase 1: a violating file records a failing gate (do NOT wipe between phases).
  printf '#[allow(dead_code)]\nfn helper() {}\n' > "$PROJ/src/x.rs"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "rust-quality-hook"' "$GATE"

  # Phase 2: same file goes clean → the assembled module clears it → passing.
  # (//! header satisfies rule_module_doc so the file is clean under all rules.)
  printf '//! X module.\nfn helper() {}\n' > "$PROJ/src/x.rs"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "passing"' "$GATE"
  jq -e '.source == "rust-quality-hook"' "$GATE"
}
