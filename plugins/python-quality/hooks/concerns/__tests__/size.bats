#!/usr/bin/env bats
# Covers the two python-quality size concerns:
#   10-size-file — file line-count limit (count_python_code_lines: blanks and
#                  full-line `#` comments excluded; docstring lines count)
#   50-size-fn   — per-def length via an indent-based span (decorators above
#                  not counted, sibling defs at the same indent, code-only count)
# Drives the ASSEMBLED registry module assembled by register.sh.

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

# --- 10-size-file ---

@test "python-quality: file at the default limit is not flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  : > ok.py
  for i in $(seq 1 400); do echo "v$i = $i" >> ok.py; done
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/ok.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "exceeds"
}

@test "python-quality: file one line over the default limit is flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  : > big.py
  for i in $(seq 1 401); do echo "v$i = $i" >> big.py; done
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/big.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "exceeds 400-line limit"
}

@test "python-quality: project config lowers maxFileLines, flags a file the default would not" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFileLines":10}}}' > .claude/toolu.config.json
  : > big.py
  for i in $(seq 1 15); do echo "v$i = $i" >> big.py; done
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/big.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "exceeds 10-line limit"
}

@test "python-quality: comment-heavy file under the code-line limit is not flagged (comment-aware count)" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFileLines":5}}}' > .claude/toolu.config.json
  : > commented.py
  for i in $(seq 1 20); do echo "# comment $i" >> commented.py; done
  echo '' >> commented.py
  echo '' >> commented.py
  for i in $(seq 1 4); do echo "v$i = $i" >> commented.py; done
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/commented.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "exceeds"
}

# --- 50-size-fn ---

@test "python-quality: def at the default fn-length limit is not flagged" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  {
    echo 'def ok():'
    for i in $(seq 1 48); do echo "    v$i = $i"; done
    echo '    return v1'
  } > ok.py
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/ok.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Function too long"
}

@test "python-quality: project config lowers maxFnLines and flags an over-limit def" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFnLines":3}}}' > .claude/toolu.config.json
  cat > m.py <<'EOF'
def big():
    a = 1
    b = 2
    c = 3
    return a + b + c
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/m.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
  echo "$output" | grep -q "big:1"
}

@test "python-quality: decorator line above def is not counted toward its length" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFnLines":4}}}' > .claude/toolu.config.json
  cat > m.py <<'EOF'
@staticmethod
def ok():
    a = 1
    b = 2
    return a + b
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/m.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Function too long"
}

@test "python-quality: method inside a class is measured to its own close, not the class's" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFnLines":3}}}' > .claude/toolu.config.json
  cat > m.py <<'EOF'
class Foo:
    def short(self):
        return 1

    def long_method(self):
        a = 1
        b = 2
        c = 3
        return a + b + c
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/m.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
  echo "$output" | grep -q "long_method"
  ! echo "$output" | grep -q "short:2"
}

@test "python-quality: async def is subject to the fn-length limit" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  _python_project
  mkdir -p .claude
  echo '{"lang":{"python":{"maxFnLines":3}}}' > .claude/toolu.config.json
  cat > m.py <<'EOF'
async def big():
    a = 1
    b = 2
    c = 3
    return a + b + c
EOF
  payload='{"tool_input":{"file_path":"'"$TMP_PROJ"'/m.py"}}'
  tool_name=Write input="$payload" PROJECT_ROOT="$TMP_PROJ" run bash "$HOOK"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Function too long"
}
