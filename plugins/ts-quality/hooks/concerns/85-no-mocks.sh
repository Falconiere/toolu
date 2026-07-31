# Check: no-mocks — tests must exercise real data/services, not mocked stand-ins.
# Scope: test files only — *.test.ts(x) / *.spec.ts(x), or any file under a
# __tests__/ directory — excluding e2e specs, which follow the platform's own
# fixture conventions (same carve-out as 20-tests.sh / 65-suppression.sh).
# Opt-out: lang.ts.noMocks=false in the merged config (default true — blocking).
if [[ "$FILE_PATH" != */e2e/* ]] && { [[ "$FILE_PATH" =~ \.(test|spec)\.(ts|tsx)$ ]] || [[ "$FILE_PATH" == */__tests__/* ]]; } \
   && [ "$(quality_flag ts noMocks true)" = "true" ]; then

  NOMOCKS_LINES=""
  if command -v ast-grep >/dev/null 2>&1; then
    # ONE ast-grep process for all the mock-call rules below, same shape as
    # 78-error-ast.sh: `scan --inline-rules` parses once and returns every
    # rule's hits as JSON. severity is `warning` on purpose — `scan` exits
    # non-zero on an ERROR-level match, which would be indistinguishable from a
    # crashed tool; at `warning` a non-zero exit (or any stderr) always means
    # the tool itself broke.
    ts_nomocks_err_file="$(mktemp)"
    ts_nomocks_rules=$(cat <<'TS_NOMOCKS_RULES'
id: jest-mock
language: ts
severity: warning
message: jest.mock() call
rule:
  pattern: jest.mock($$$)
---
id: vi-mock
language: ts
severity: warning
message: vi.mock() call
rule:
  pattern: vi.mock($$$)
---
id: jest-fn
language: ts
severity: warning
message: jest.fn() call
rule:
  pattern: jest.fn($$$)
---
id: vi-fn
language: ts
severity: warning
message: vi.fn() call
rule:
  pattern: vi.fn($$$)
---
id: sinon-method
language: ts
severity: warning
message: sinon mock method call
rule:
  pattern: sinon.$M($$$)
TS_NOMOCKS_RULES
)

    ts_nomocks_failed=0
    ts_nomocks_fail_stage=""
    ts_nomocks_json=$(ast-grep scan --inline-rules "$ts_nomocks_rules" --json "$FILE_PATH" 2>"$ts_nomocks_err_file")
    ts_nomocks_rc=$?
    if [[ "$ts_nomocks_rc" -ne 0 || -s "$ts_nomocks_err_file" ]]; then
      ts_nomocks_failed=1
      ts_nomocks_fail_stage="ast-grep"
    elif [[ -z "$(printf '%s' "$ts_nomocks_json" | tr -d '[:space:]')" ]]; then
      # ast-grep exited 0 with genuinely empty stdout — jq also exits 0 on
      # empty input with empty output, so left unchecked this would be
      # indistinguishable from a clean "[]" scan and silently swallow a real
      # breakage as "no hits". A well-behaved scan always emits at least "[]".
      ts_nomocks_failed=1
      ts_nomocks_fail_stage="empty-output"
    elif ! NOMOCKS_LINES=$(printf '%s' "$ts_nomocks_json" | jq -r '
          .[] | . as $m
          | ((($m.lines // "") | split("\n")) | to_entries[])
          | "\($m.ruleId)\t\($m.file):\($m.range.start.line + 1 + .key):\(.value)"
        ' 2>/dev/null); then
      ts_nomocks_failed=1
      ts_nomocks_fail_stage="jq"
      NOMOCKS_LINES=""
    fi

    if [[ "$ts_nomocks_failed" -ne 0 ]]; then
      ts_nomocks_stderr_first=$(head -n 1 "$ts_nomocks_err_file" 2>/dev/null | cut -c1-200)
      if [[ "$ts_nomocks_fail_stage" == "jq" ]]; then
        add_error "ast-grep failed while scanning $FILE_PATH for mocks — its output did not parse as the documented JSON array; no-mocks rule could not be verified. Fix the tool/file and re-edit"
      elif [[ "$ts_nomocks_fail_stage" == "empty-output" ]]; then
        add_error "ast-grep failed while scanning $FILE_PATH for mocks — exited 0 with empty output (expected at least the JSON array \"[]\"); no-mocks rule could not be verified. Fix the tool/file and re-edit"
      else
        add_error "ast-grep failed while scanning $FILE_PATH for mocks — exit ${ts_nomocks_rc}${ts_nomocks_stderr_first:+: $ts_nomocks_stderr_first}; no-mocks rule could not be verified. Fix the tool/file and re-edit"
      fi
    fi
    rm -f "$ts_nomocks_err_file"

    if [[ -n "$NOMOCKS_LINES" ]]; then
      # Strip the leading "<ruleId>\t" — only the excerpt (file:line:source) is shown.
      NOMOCKS_EXCERPT=$(printf '%s\n' "$NOMOCKS_LINES" | cut -f2- | head -5)
      add_error "Mocked test double in $FILE_PATH — tests must exercise real data/services, not mocks/stubs (jest.mock/vi.mock/jest.fn/vi.fn/sinon)"$'\n'"${NOMOCKS_EXCERPT}"
    fi
  fi
  # ast-grep absent: the mock-call rules above are skipped, same fail-soft
  # posture as 78-error-ast.sh — but the ts-mockito import check below needs no
  # external tool, so it still runs.

  # ts-mockito is a whole mocking LIBRARY imported by name — a plain grep for
  # the import specifier, same idiom as the `../` import check in 10-imports.sh
  # (quote-agnostic via a bracket class).
  if grep -qE "from[[:space:]]+[\"']ts-mockito[\"']" "$FILE_PATH" 2>/dev/null; then
    add_error "Import from ts-mockito in $FILE_PATH — tests must exercise real data/services, not mocks/stubs"
  fi
fi

