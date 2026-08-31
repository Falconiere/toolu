#!/usr/bin/env bats
# Covers the python-quality 30-suppression concern: bare `except:`, one-line
# `except ...: pass`, blanket `# noqa`, blanket `# type: ignore` — and their
# scoped counterparts, which must pass cleanly (zero false positives).

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

@test "python-quality: bare except: is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept:\n    log_it()\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden suppression"
  echo "$output" | grep -q "bare except:"
}

@test "python-quality: one-line except ...: pass is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept ValueError: pass\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "one-line except"
}

@test "python-quality: blanket # noqa is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'x = 1  # noqa\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "blanket # noqa"
}

@test "python-quality: blanket # type: ignore is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'x: int = "y"  # type: ignore\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "blanket # type: ignore"
}

@test "python-quality: except SpecificError: with an indented body passes" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'try:\n    pass\nexcept ValueError:\n    log_it()\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden suppression"
}

@test "python-quality: scoped # noqa: E501 passes" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'x = 1  # noqa: E501\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden suppression"
}

@test "python-quality: scoped # type: ignore[arg-type] passes" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  printf 'x: int = "y"  # type: ignore[assignment]\n' > m.py
  _check m.py
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden suppression"
}
