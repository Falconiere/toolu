# Test placement: colocated `test_*.py` / `*_test.py` convention (module-sibling,
# not a separate tests/ tree — the TS __tests__/ colocation principle, applied to
# Python's own naming idiom). Two rules:
#   (a) a test-bearing file (top-level `def test_...`/`async def test_...`, or a
#       pytest/unittest import) whose basename does not follow the convention.
#   (b) a `test_*.py` sitting in a directory with no non-test .py sibling — it
#       has nothing to be "colocated" with.
# `conftest.py` (pytest's fixture-sharing file — imports pytest but is not
# itself a test) and `__init__.py` (package marker, never a test) are exempt
# from both rules.
_py_base="$(basename "$FILE_PATH")"
_is_python_test_file=0
case "$_py_base" in
  test_*.py|*_test.py) _is_python_test_file=1 ;;
esac

case "$_py_base" in
  conftest.py|__init__.py) ;;
  *)
    _has_test_marker=0
    grep -qE '^(async[[:space:]]+)?def[[:space:]]+test_[A-Za-z0-9_]*[[:space:]]*\(' "$FILE_PATH" 2>/dev/null && _has_test_marker=1
    grep -qE '^(import|from)[[:space:]]+(pytest|unittest)\b' "$FILE_PATH" 2>/dev/null && _has_test_marker=1
    if [[ "$_has_test_marker" -eq 1 && "$_is_python_test_file" -eq 0 ]]; then
      add_error "Test-bearing file not named test_*.py or *_test.py: $FILE_PATH — rename to follow the colocated test convention"
    fi

    if [[ "$_py_base" == test_*.py ]]; then
      _py_dir="$(dirname "$FILE_PATH")"
      if ! find "$_py_dir" -maxdepth 1 -type f -name '*.py' \
          ! -name 'test_*.py' ! -name '*_test.py' ! -name 'conftest.py' ! -name '__init__.py' \
          2>/dev/null | grep -q .; then
        add_error "Test not co-located with a module: $FILE_PATH — colocate test_*.py next to the module it tests"
      fi
    fi
    ;;
esac

