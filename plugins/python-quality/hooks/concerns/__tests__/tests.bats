#!/usr/bin/env bats
# Covers the python-quality 20-tests concern: the colocated `test_*.py` /
# `*_test.py` naming convention (rule a), the "must have a non-test sibling"
# colocation check (rule b), and the conftest.py/__init__.py exemptions.

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

# --- rule (a): naming convention ---

@test "python-quality: a top-level def test_ in a misnamed file is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def test_something():\n    assert True\n' > checks.py
  _check checks.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "not named test_\*.py or \*_test.py"
}

@test "python-quality: a pytest import in a misnamed file is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'import pytest\n\n\ndef check_thing():\n    assert True\n' > checks.py
  _check checks.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "not named test_\*.py or \*_test.py"
}

@test "python-quality: a unittest import in a misnamed file is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'from unittest import TestCase\n' > checks.py
  _check checks.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "not named test_\*.py or \*_test.py"
}

@test "python-quality: test_foo.py naming is accepted" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def module():\n    return 1\n' > foo.py
  printf 'def test_something():\n    assert True\n' > test_foo.py
  _check test_foo.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "not named test_"
}

@test "python-quality: foo_test.py naming is accepted" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'def module():\n    return 1\n' > foo.py
  printf 'def test_something():\n    assert True\n' > foo_test.py
  _check foo_test.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "not named test_"
}

# --- rule (b): must be colocated next to a non-test sibling ---

@test "python-quality: test_*.py alone in its directory (no module sibling) is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p lonely
  printf 'def test_something():\n    assert True\n' > lonely/test_orphan.py
  _check lonely/test_orphan.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Test not co-located with a module"
}

@test "python-quality: test_*.py next to its module passes" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p pkg
  printf 'def add(a, b):\n    return a + b\n' > pkg/calc.py
  printf 'def test_add():\n    assert True\n' > pkg/test_calc.py
  _check pkg/test_calc.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Test not co-located"
}

# --- exemptions ---

@test "python-quality: conftest.py importing pytest is exempt from the naming rule" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p lonely
  printf 'import pytest\n\n\n@pytest.fixture\ndef thing():\n    """A fixture."""\n    return 1\n' > lonely/conftest.py
  _check lonely/conftest.py
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "python-quality: __init__.py is exempt from the naming rule" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p pkg
  printf 'import unittest\n' > pkg/__init__.py
  _check pkg/__init__.py
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
