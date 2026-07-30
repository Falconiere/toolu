# E2E specs follow a framework layout (e2e/specs/<feature>/*.spec.ts), not the
# unit-test __tests__/ co-location model — exempt them from these rules.
if [[ "$FILE_PATH" =~ \.(test|spec)\.(ts|tsx)$ && "$FILE_PATH" != */e2e/* ]]; then
  if [[ "$FILE_PATH" != */__tests__/* ]]; then
    add_error "Test file outside __tests__/: $FILE_PATH — move to sibling __tests__/ directory"
  else
    _after_tests="${FILE_PATH##*__tests__/}"
    if [[ "$_after_tests" == */* ]]; then
      _subdir="${_after_tests%%/*}"
      if [[ "$_subdir" != "fixtures" && "$_subdir" != "helpers" && "$_subdir" != "utils" ]]; then
        add_error "Test nested in __tests__/ subdirectory: $FILE_PATH — keep __tests__/ flat (only fixtures/helpers/utils subdirs allowed; no mocks/ — tests must exercise real data)"
      fi
    fi
    # `%` (shortest suffix) resolves to the NEAREST __tests__, matching the
    # `##` (last __tests__) used for the nested-subdir check above; `%%` would
    # pick the OUTERMOST __tests__ and mis-validate a doubly-nested path.
    _tests_dir="${FILE_PATH%__tests__/*}__tests__"
    _parent_dir=$(dirname "$_tests_dir")
    if ! find "$_parent_dir" -maxdepth 1 -type f \( -name "*.ts" -o -name "*.tsx" \) \
        ! -name "*.test.*" ! -name "*.spec.*" ! -name "*.d.ts" 2>/dev/null | grep -q . \
       && ! find "$_parent_dir" -maxdepth 1 -type d ! -name "__tests__" ! -name "." 2>/dev/null | grep -q .; then
      add_error "Test not co-located with source: $FILE_PATH — __tests__/ must be at the same level as the code it tests"
    fi
  fi
fi

