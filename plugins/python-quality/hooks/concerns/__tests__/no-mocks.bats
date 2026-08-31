#!/usr/bin/env bats
# Covers the python-quality 70-no-mocks concern: mock imports (unittest.mock,
# the standalone mock package, pytest_mock) and mocker/monkeypatch fixture
# parameters, scoped to test files only, opt-out via lang.python.noMocks.

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

@test "python-quality: from unittest import mock in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'from unittest import mock\n\n\ndef test_it():\n    """Test it."""\n    assert mock\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mock import"
}

@test "python-quality: import unittest.mock in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'import unittest.mock\n\n\ndef test_it():\n    """Test it."""\n    assert unittest.mock\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mock import"
}

@test "python-quality: from mock import Mock in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'from mock import Mock\n\n\ndef test_it():\n    """Test it."""\n    assert Mock\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mock import"
}

@test "python-quality: import pytest_mock in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'import pytest_mock\n\n\ndef test_it():\n    """Test it."""\n    assert pytest_mock\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mock import"
}

@test "python-quality: from unittest.mock import MagicMock, patch in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'from unittest.mock import MagicMock, patch\n\n\ndef test_it():\n    """Test it."""\n    assert MagicMock and patch\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mock import"
}

@test "python-quality: mocker fixture parameter in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'def test_it(mocker):\n    """Test it."""\n    assert mocker\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mocker/monkeypatch fixture parameter"
}

@test "python-quality: monkeypatch fixture parameter in a test file is flagged" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'def test_it(monkeypatch):\n    """Test it."""\n    assert monkeypatch\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no-mocks: mocker/monkeypatch fixture parameter"
}

@test "python-quality: mock import in a NON-test file is not flagged (scope is test files only)" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  # "import mock" (not "unittest"/"pytest") so 20-tests.sh's naming-convention
  # check does not also fire here — this test isolates the no-mocks scope rule.
  printf 'import mock\n\n\ndef helper():\n    """Use mock."""\n    return mock\n' > helper.py
  _check helper.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}

@test "python-quality: mock import in a test file passes when lang.python.noMocks is false" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"noMocks":false}}}' > .claude/toolu.config.json
  printf 'from unittest import mock\n\n\ndef test_it():\n    """Test it."""\n    assert mock\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}

@test "python-quality: a real-fixture test with no mock usage passes clean" {
  command -v ast-grep >/dev/null 2>&1 || skip "ast-grep not installed"
  _python_project
  printf 'def test_add():\n    """Add two numbers."""\n    assert 1 + 1 == 2\n' > test_a.py
  _check test_a.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "no-mocks"
}
