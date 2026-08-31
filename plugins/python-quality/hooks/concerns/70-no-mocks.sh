# --- No-mocks (zero tolerance): forbid mock frameworks — real data/fixtures
# only. Test files ONLY (test_*.py / *_test.py / conftest.py, per the shared
# $_is_python_test_file set in 20-tests.sh) — a no-op elsewhere, mirroring
# rust's test-scope split in 70-no-mocks.sh. Same ast-grep-scan pattern: ONE
# process, severity warning (a matching rule and a clean run both exit 0 —
# only a genuine crash is distinguishable from a clean pass), jq flattens
# every hit to its def/import line, add_error on a match or on tool breakage.
#
# Opt-out: lang.python.noMocks (default true) via quality_flag.
if [[ "${_is_python_test_file:-0}" -eq 1 ]] \
   && [[ "$(quality_flag python noMocks true)" == "true" ]] \
   && command -v ast-grep >/dev/null 2>&1; then
  NM_RULES=$(cat <<'PY_NOMOCK_RULES'
id: mock-import
language: python
severity: warning
message: 'mock import'
rule:
  any:
    - kind: import_statement
      has: {kind: dotted_name, regex: '^(unittest\.mock|mock|pytest_mock)$'}
    - kind: import_from_statement
      has: {field: module_name, kind: dotted_name, regex: '^(unittest\.mock|mock)$'}
    - kind: import_from_statement
      all:
        - has: {field: module_name, kind: dotted_name, regex: '^unittest$'}
        - has: {field: name, kind: dotted_name, regex: '^mock$'}
---
id: mocker-param
language: python
severity: warning
message: 'mocker/monkeypatch fixture parameter'
rule:
  kind: function_definition
  has:
    field: parameters
    has: {kind: identifier, regex: '^(mocker|monkeypatch)$'}
PY_NOMOCK_RULES
)

  nm_err_file="$(mktemp)"
  nm_json=$(ast-grep scan --inline-rules "$NM_RULES" --json "$FILE_PATH" 2>"$nm_err_file")
  nm_rc=$?
  nm_grep_failed=0
  nm_fail_detail=""
  nm_lines=""
  if [[ "$nm_rc" -ne 0 || -s "$nm_err_file" ]]; then
    nm_grep_failed=1
    nm_stderr_first=$(head -n 1 "$nm_err_file" 2>/dev/null | cut -c1-200)
    nm_fail_detail="ast-grep exit ${nm_rc}${nm_stderr_first:+: $nm_stderr_first}"
  elif [[ -z "$(printf '%s' "$nm_json" | tr -d '[:space:]')" ]]; then
    nm_grep_failed=1
    nm_fail_detail="ast-grep exited 0 with empty output (expected at least the JSON array \"[]\")"
  elif ! nm_lines=$(printf '%s' "$nm_json" | jq -r '
        .[] | "\(.ruleId)\t\(.range.start.line + 1): \(.lines | split("\n")[0])"
      ' 2>/dev/null); then
    nm_grep_failed=1
    nm_fail_detail="ast-grep exited 0 but its output did not parse as the documented JSON array"
    nm_lines=""
  fi
  rm -f "$nm_err_file"

  IMPORT_HITS=$(printf '%s\n' "$nm_lines" | awk -F'\t' '$1=="mock-import"{print $2}' | head -5)
  if [[ -n "$IMPORT_HITS" ]]; then
    add_error "no-mocks: mock import in $FILE_PATH — write against real data/fixtures instead"$'\n'"${IMPORT_HITS}"
  fi
  PARAM_HITS=$(printf '%s\n' "$nm_lines" | awk -F'\t' '$1=="mocker-param"{print $2}' | head -5)
  if [[ -n "$PARAM_HITS" ]]; then
    add_error "no-mocks: mocker/monkeypatch fixture parameter in $FILE_PATH — write against real data/fixtures instead"$'\n'"${PARAM_HITS}"
  fi

  if [[ "$nm_grep_failed" -ne 0 ]]; then
    add_error "ast-grep failed while scanning $FILE_PATH — ${nm_fail_detail:-unknown error}; no-mocks rules could not be verified. Fix the tool/file and re-edit"
  fi
fi

