# Check: console.log — capture once, then test (not grep-twice).
CONSOLE_LINES=$(grep -nE '^[[:space:]]*console\.log\(' "$FILE_PATH" 2>/dev/null | grep -vE '^[0-9]+:[[:space:]]*//' | head -3)
if [[ -n "$CONSOLE_LINES" ]]; then
  add_error "Forbidden console.log in $FILE_PATH — use console.error/warn/info"$'\n'"${CONSOLE_LINES}"
fi

