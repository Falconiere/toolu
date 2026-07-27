# Check: manual try/catch+toast pattern in components
if [[ "$FILE_PATH" == */components/* || "$FILE_PATH" == */routes/* ]]; then
  # [[:space:]] not \s: BSD awk (macOS) has no \s and reads it as a literal `s`,
  # which silently killed this rule for the universal `catch (` spelling.
  CATCH_TOAST=$(awk '/catch[[:space:]]*\(/{found=1} found && /toast\(/{print NR": "$0; found=0}' "$FILE_PATH" 2>/dev/null | head -3)
  if [[ -n "$CATCH_TOAST" ]]; then
    add_error "Manual try/catch+toast in $FILE_PATH — use shared error handling"$'\n'"${CATCH_TOAST}"
  fi
fi

