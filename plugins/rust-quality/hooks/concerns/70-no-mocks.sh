# --- No-mocks (zero tolerance): forbid mock frameworks — real data/fixtures only ---
# Split scope, because mock *definitions* live in src/ while mock *imports* are
# pulled into test files:
#   - src/**/*.rs: #[automock], #[cfg_attr(..., automock)], mock! { ... }
#     (also the mock!(...) and mock![...] delimiter forms — mock! is a macro,
#     so all three bracket kinds are valid Rust).
#   - test files (tests/ placement, or *_test.rs / *_tests.rs basenames — the
#     same path predicate 20-tests.sh uses; its attribute-based detection
#     (#[test], #[rstest], ...) is not reused here since mock imports are `use`
#     statements, not attribute-marked items): use mockall::...;, use faux::...;.
# Same ast-grep-scan pattern as 60-error-handling.sh: ONE process, severity
# warning (a matching rule and a clean run both exit 0 — only a genuine crash
# is distinguishable from a clean pass), jq flattens every hit to one excerpt
# line, add_error on a match or on tool breakage.
#
# Opt-out: lang.rust.noMocks (default true) via quality_flag.
if [[ "$(quality_flag rust noMocks true)" == "true" ]] && command -v ast-grep >/dev/null 2>&1; then
  _nm_in_src=0
  [[ "$FILE_PATH" == */src/* ]] && _nm_in_src=1

  _nm_is_test_file=0
  [[ "$FILE_PATH" == */tests/* ]] && _nm_is_test_file=1
  case "$(basename "$FILE_PATH")" in
    *_test.rs|*_tests.rs) _nm_is_test_file=1 ;;
  esac

  NM_RULES=""
  if [[ "$_nm_in_src" -eq 1 ]]; then
    NM_RULES=$(cat <<'RS_NOMOCK_SRC'
id: automock-attr
language: rust
severity: warning
message: '#[automock] mock definition'
rule:
  pattern: '#[automock]'
---
id: automock-cfg-attr
language: rust
severity: warning
message: cfg_attr(..., automock) mock definition
rule:
  pattern: '#[cfg_attr($$$, automock)]'
---
id: mock-macro
language: rust
severity: warning
message: 'mock! { ... } mock definition'
rule:
  any:
    - pattern: 'mock! { $$$ }'
    - pattern: 'mock!($$$)'
    - pattern: 'mock![$$$]'
RS_NOMOCK_SRC
)
  fi
  if [[ "$_nm_is_test_file" -eq 1 ]]; then
    NM_TEST_RULES=$(cat <<'RS_NOMOCK_TEST'
id: mockall-import
language: rust
severity: warning
message: 'use mockall::...; mock import'
rule:
  kind: use_declaration
  regex: 'mockall::'
---
id: faux-import
language: rust
severity: warning
message: 'use faux::...; mock import'
rule:
  kind: use_declaration
  regex: 'faux::'
RS_NOMOCK_TEST
)
    if [[ -n "$NM_RULES" ]]; then
      NM_RULES="${NM_RULES}"$'\n---\n'"${NM_TEST_RULES}"
    else
      NM_RULES="$NM_TEST_RULES"
    fi
  fi

  if [[ -n "$NM_RULES" ]]; then
    nm_err_file="$(mktemp)"
    nm_grep_failed=0
    nm_fail_detail=""
    nm_lines=""
    nm_json=$(ast-grep scan --inline-rules "$NM_RULES" --json "$FILE_PATH" 2>"$nm_err_file")
    nm_rc=$?
    # Two distinct failure modes, reported distinctly: ast-grep itself broke, or
    # it exited 0 and returned something that is not the documented JSON array.
    if [[ "$nm_rc" -ne 0 || -s "$nm_err_file" ]]; then
      nm_grep_failed=1
      nm_stderr_first=$(head -n 1 "$nm_err_file" 2>/dev/null | cut -c1-200)
      nm_fail_detail="ast-grep exit ${nm_rc}${nm_stderr_first:+: $nm_stderr_first}"
    elif [[ -z "$(printf '%s' "$nm_json" | tr -d '[:space:]')" ]]; then
      # ast-grep exited 0 with genuinely empty stdout — jq also exits 0 on
      # empty input with empty output, so left unchecked this would be
      # indistinguishable from a clean "[]" scan and silently swallow a real
      # breakage as "no hits". A well-behaved scan always emits at least "[]".
      nm_grep_failed=1
      nm_fail_detail="ast-grep exited 0 with empty output (expected at least the JSON array \"[]\")"
    elif ! nm_lines=$(printf '%s' "$nm_json" | jq -r '
          .[] | . as $m
          | ((($m.lines // "") | split("\n")) | to_entries[])
          | "\($m.ruleId)\t\($m.file):\($m.range.start.line + 1 + .key):\(.value)"
        ' 2>/dev/null); then
      nm_grep_failed=1
      nm_fail_detail="ast-grep exited 0 but its output did not parse as the documented JSON array"
      nm_lines=""
    fi

    # nm_hits RULE_ID LIMIT — this rule's excerpt lines, capped.
    nm_hits() {
      local _id="$1" _limit="$2" _row _rest
      [ -n "$nm_lines" ] || return 0
      while IFS=$'\t' read -r _row _rest; do
        [ "$_row" = "$_id" ] && printf '%s\n' "$_rest"
      done <<< "$nm_lines" | head -n "$_limit"
    }

    AUTOMOCK_HITS=$(nm_hits automock-attr 5; nm_hits automock-cfg-attr 5)
    if [[ -n "$AUTOMOCK_HITS" ]]; then
      add_error "no-mocks: #[automock] mock definition in $FILE_PATH — write against real data/fixtures instead"$'\n'"${AUTOMOCK_HITS}"
    fi
    MOCK_MACRO_HITS=$(nm_hits mock-macro 5)
    if [[ -n "$MOCK_MACRO_HITS" ]]; then
      add_error "no-mocks: mock! { ... } mock definition in $FILE_PATH — write against real data/fixtures instead"$'\n'"${MOCK_MACRO_HITS}"
    fi
    MOCK_IMPORT_HITS=$(nm_hits mockall-import 5; nm_hits faux-import 5)
    if [[ -n "$MOCK_IMPORT_HITS" ]]; then
      add_error "no-mocks: mockall/faux import in $FILE_PATH — write against real data/fixtures instead"$'\n'"${MOCK_IMPORT_HITS}"
    fi

    rm -f "$nm_err_file"
    if [[ "$nm_grep_failed" -ne 0 ]]; then
      add_error "ast-grep failed while scanning $FILE_PATH — ${nm_fail_detail:-unknown error}; no-mocks rules could not be verified. Fix the tool/file and re-edit"
    fi
  fi
fi
