# Check: suppression comments — fix the issue in code, never silence the tool.
# @ts-expect-error is exempt in test/spec files, where asserting a compile error
# is a legitimate test; banned everywhere else like the rest.
SUPPRESS_TOKENS='@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore'
if [[ ! "$FILE_PATH" =~ \.(test|spec)\.(ts|tsx)$ ]]; then
  SUPPRESS_TOKENS="${SUPPRESS_TOKENS}|@ts-expect-error"
fi
DISABLE_LINES=$(grep -nE "(//|/\*+)[[:space:]]*(${SUPPRESS_TOKENS})" "$FILE_PATH" 2>/dev/null | head -3)
if [[ -n "$DISABLE_LINES" ]]; then
  add_error "Forbidden suppression comment in $FILE_PATH — fix the underlying issue in code, never silence it"$'\n'"${DISABLE_LINES}"
fi

