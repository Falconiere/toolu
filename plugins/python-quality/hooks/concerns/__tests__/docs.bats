#!/usr/bin/env bats
# Covers the python-quality 90-docs concern: top-level, non-underscore def/class
# must open with a docstring — soft advisory only, never blocks the gate.

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

_check() {
  local file="$1"
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ/$file"'"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
}

@test "python-quality: top-level def missing a docstring is an advisory (does not fail the gate)" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def undocumented():\n    return 1\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "missing a docstring"
  echo "$output" | grep -q "undocumented"
  ! jq -e '.status == "failing"' "$TMP_PROJ/.claude/tmp/quality-gate-status.json" 2>/dev/null
}

@test "python-quality: top-level def with a docstring is not flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def documented():\n    """Does a thing."""\n    return 1\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "missing a docstring"
}

@test "python-quality: top-level def with a single-quoted docstring is not flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf "def documented():\n    '''Does a thing.'''\n    return 1\n" > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "missing a docstring"
}

@test "python-quality: a _private top-level def is exempt" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def _private():\n    return 1\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "missing a docstring"
}

@test "python-quality: an indented method without a docstring is exempt (top-level only)" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  cat > m.py <<'EOF'
class Foo:
    """A class."""

    def method(self):
        return 1
EOF
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "missing a docstring"
}

@test "python-quality: a class without a docstring is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'class Foo:\n    def method(self):\n        return 1\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "missing a docstring"
  echo "$output" | grep -q "Foo"
}

@test "python-quality: a multi-line signature is tracked to its closing colon before the docstring check" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  cat > m.py <<'EOF'
def multi_line(
    a,
    b,
):
    """Adds two numbers."""
    return a + b
EOF
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "missing a docstring"
}

@test "python-quality: a multi-line signature missing its docstring is still flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  cat > m.py <<'EOF'
def multi_line(
    a,
    b,
):
    return a + b
EOF
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "missing a docstring"
  echo "$output" | grep -q "multi_line"
}
