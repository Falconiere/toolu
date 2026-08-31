#!/usr/bin/env bats
# Covers the python-quality 00-preamble (project/lib detection, file-path
# extraction across Write/Edit/MultiEdit, delete/move gate-clear) and
# 99-finalize (multi-slot gate write/clear) concerns. Drives the ASSEMBLED
# registry module — register.sh concatenates concerns/[0-9][0-9]-*.sh into one
# runtime script. Also proves the module no-ops in a non-Python repo.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

setup() {
  TMP=$(mktemp -d)

  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  HOOK="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/python-quality@toolu__python-quality.sh"

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

_python_project() {
  printf '[project]\nname = "x"\nversion = "0.1.0"\n' > pyproject.toml
  git add -A
  git -c user.email=t@t -c user.name=t commit -q -m setup
}

@test "python-quality: no-op outside a Python project (no pyproject.toml/setup.py/etc.)" {
  payload='{"tool_input":{"file_path":"/nonexistent.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "python-quality: Edit tool extracts file path and flags violations (regression)" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    pass\n' > bad.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/bad.py"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden suppression"
}

# Regression: the PostToolUse matcher includes MultiEdit, but the file-path
# extraction only ran for Write/Edit — a MultiEdit on a .py file (with
# CLAUDE_FILE_PATHS unset) silently skipped all quality checks.
@test "python-quality: MultiEdit extracts file path and flags violations (regression)" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    pass\n' > bad.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/bad.py"}}'
  tool_name=MultiEdit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden suppression"
}

@test "python-quality: gate is cleared when the failing file is re-edited clean" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    pass\n' > bad.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/bad.py"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"

  printf 'try:\n    pass\nexcept ValueError:\n    pass\n' > bad.py
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -e '.source == "python-quality-hook"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json"
}

@test "python-quality: deleting a failing file clears its gate entry" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    pass\n' > bad.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/bad.py"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json" >/dev/null

  rm "$TMP_PROJ/bad.py"
  TOOLU_EDIT_OPERATION=delete tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json" >/dev/null
}

@test "python-quality: exits 0 silently when TOOLU_LIB_DIR is unset (fail soft)" {
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/main.py"}}'
  run env -u TOOLU_LIB_DIR tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "python-quality: clearing one file does not clobber another file's failure" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    pass\n' > a.py
  printf 'try:\n    pass\nexcept:\n    pass\n' > b.py
  payload_a='{"tool_input":{"file_path":"'"$TMP_PROJ"'/a.py"}}'
  payload_b='{"tool_input":{"file_path":"'"$TMP_PROJ"'/b.py"}}'
  GATE="$TMP_PROJ/.claude/tmp/quality-gate-status.json"

  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"

  # b.py goes clean — a.py's violation must survive and keep the gate failing.
  printf 'try:\n    pass\nexcept ValueError:\n    pass\n' > b.py
  tool_name=Edit input="$payload_b" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e --arg f "$TMP_PROJ/a.py" '.entries[$f]' "$GATE"

  # a.py goes clean too — now the gate may pass.
  printf 'try:\n    pass\nexcept ValueError:\n    pass\n' > a.py
  tool_name=Edit input="$payload_a" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "passing"' "$GATE"
}

@test "python-quality: failing gate owned by another hook survives a python fail->clear cycle" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p "$TMP_PROJ/.claude/tmp"
  GATE="$TMP_PROJ/.claude/tmp/quality-gate-status.json"
  jq -n '{status: "failing", reason: "Rust violation", source: "rust-quality-hook",
          file: "/p/x.rs", violations: "bad rust\n", updatedAt: "2026-01-01T00:00:00Z"}' > "$GATE"

  printf 'try:\n    pass\nexcept:\n    pass\n' > a.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/a.py"}}'
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "python-quality-hook"' "$GATE"

  # Python file goes clean — the Rust failure must be promoted back, not erased.
  printf 'try:\n    pass\nexcept ValueError:\n    pass\n' > a.py
  tool_name=Edit input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "rust-quality-hook"' "$GATE"
  jq -e '.reason == "Rust violation"' "$GATE"
}
