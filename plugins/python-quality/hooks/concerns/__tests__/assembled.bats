#!/usr/bin/env bats
# Equivalence/behavior harness for the python-quality plugin's ASSEMBLED
# registry module. There is no pre-split monolith for python-quality (unlike
# rust-quality/ts-quality, it was authored fragment-first) — these tests
# instead prove: (1) the assembled module is byte-identical to a manual
# ordered concatenation of the numbered fragments (catches a register.sh
# ordering/clobber regression), and (2) the assembled module's gate-write
# behavior is internally consistent (multi-violation ordering, no-clobber on
# partial fix, suppression, clean-file clearing) — the same properties the
# rust/ts oracle-diff tests check against their monolith.

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
  ASSEMBLED="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/python-quality@toolu__python-quality.sh"
  CONCERNS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"

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

# Minimal Python project so detect_python passes.
_python_project() {
  printf '[project]\nname = "x"\nversion = "0.1.0"\n' > "$PROJ/pyproject.toml"
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

# Run a given hook script on a fixture file; print the resulting gate JSON.
_run_gate() {
  local hook="$1" file="$2"
  rm -rf "$PROJ/.claude/tmp"
  local payload='{"tool_input":{"file_path":"'"$file"'"}}'
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Write input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$hook" >/dev/null 2>&1
  local gate="$PROJ/.claude/tmp/quality-gate-status.json"
  if [ -f "$gate" ]; then cat "$gate"; fi
}

_gate_canon() {
  _run_gate "$1" "$2" | jq -S 'del(.updatedAt) | (.entries // {}) |= with_entries(.value |= del(.updatedAt))'
}

@test "assembled module exists after register.sh" {
  [ -f "$ASSEMBLED" ]
}

@test "python assembled module == ordered concatenation of numbered fragments" {
  local manual="$TMP/manual.sh"
  : > "$manual"
  for f in "$CONCERNS_DIR"/[0-9][0-9]-*.sh; do
    cat "$f" >> "$manual"
    printf '\n' >> "$manual"
  done
  # register.sh writes the SAME bytes it would concatenate manually.
  diff "$manual" "$ASSEMBLED"
}

# --- 2-violation fixture: ordering + no clobber ---
@test "python assembled: 2-violation fixture reports both, in fragment order" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  mkdir -p "$PROJ/.claude"
  echo '{"lang":{"python":{"maxFileLines":5}}}' > "$PROJ/.claude/toolu.config.json"
  {
    echo 'def big():'
    echo '    try:'
    echo '        pass'
    echo '    except:'
    for i in $(seq 1 6); do echo "        v$i = $i"; done
  } > "$PROJ/bad.py"

  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/bad.py")
  echo "$asm" | jq -e '.status == "failing"'
  echo "$asm" | jq -e '.violations | contains("exceeds 5-line limit")'
  echo "$asm" | jq -e '.violations | contains("bare except")'
  # Ordered: file-size rule (10-size-file) fires before suppression (30).
  v=$(echo "$asm" | jq -r '.violations')
  before_size="${v%%exceeds 5-line limit*}"
  before_except="${v%%bare except*}"
  [ "${#before_size}" -lt "${#before_except}" ]
}

@test "python assembled: fixing one of two violations keeps the gate failing on the other" {
  _python_project
  mkdir -p "$PROJ/.claude"
  echo '{"lang":{"python":{"maxFileLines":5}}}' > "$PROJ/.claude/toolu.config.json"
  {
    echo 'def big():'
    echo '    except_marker = 1'
    for i in $(seq 1 6); do echo "    v$i = $i"; done
    echo '    # noqa'
  } > "$PROJ/bad.py"
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/bad.py")
  echo "$asm" | jq -e '.status == "failing"'
  echo "$asm" | jq -e '.violations | contains("blanket # noqa")'

  # Fix ONLY the noqa marker — the file is still over the 5-line limit.
  {
    echo 'def big():'
    echo '    except_marker = 1'
    for i in $(seq 1 6); do echo "    v$i = $i"; done
  } > "$PROJ/bad.py"
  still=$(_gate_canon "$ASSEMBLED" "$PROJ/bad.py")
  echo "$still" | jq -e '.status == "failing"'
  echo "$still" | jq -e '.violations | contains("exceeds 5-line limit")'
  echo "$still" | jq -e '.violations | (contains("blanket # noqa") | not)'
}

@test "python assembled: clean file writes no failure (gate passing)" {
  _python_project
  cat > "$PROJ/good.py" <<'EOF'
def read_it():
    """Read and return a constant."""
    return 1
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/good.py")
  [ -z "$asm" ] || echo "$asm" | jq -e '.status != "failing"'
}

@test "python assembled: re-editing a failing file clean flips the gate to passing" {
  _python_project
  GATE="$PROJ/.claude/tmp/quality-gate-status.json"
  payload='{"tool_input":{"file_path":"'"$PROJ"'/x.py"}}'

  printf 'def helper():\n    except_bad = 1\n    return except_bad\n\n\ntry:\n    pass\nexcept:\n    pass\n' > "$PROJ/x.py"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "python-quality-hook"' "$GATE"

  printf 'def helper():\n    """Do a thing."""\n    return 1\n' > "$PROJ/x.py"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "passing"' "$GATE"
  jq -e '.source == "python-quality-hook"' "$GATE"
}
