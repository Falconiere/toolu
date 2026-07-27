# Check: confirm()/alert() in frontend files
if [[ "$FILE_PATH" == */components/* || "$FILE_PATH" == */routes/* ]]; then
  CONFIRM_LINES=$(grep -nE '\b(confirm|alert)[[:space:]]*\(' "$FILE_PATH" 2>/dev/null | grep -vE '^[0-9]+:[[:space:]]*//' | grep -vE '(ConfirmDeleteAlert|AlertDialog|customAlert|customConfirm)' | head -3)
  if [[ -n "$CONFIRM_LINES" ]]; then
    add_error "Forbidden confirm()/alert() in $FILE_PATH — use AlertDialog component"$'\n'"${CONFIRM_LINES}"
  fi
fi

