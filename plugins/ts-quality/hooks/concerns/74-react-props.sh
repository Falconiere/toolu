# Check: mutable props (Props type param without Readonly)
PROPS_LINES=$(grep -nE '\((props|[a-z]+Props):[[:space:]]+[A-Z][a-zA-Z]+Props\)' "$FILE_PATH" 2>/dev/null | grep -v 'Readonly' | head -3)
if [[ -n "$PROPS_LINES" ]]; then
  add_error "Mutable props in $FILE_PATH — wrap in Readonly<Props>"$'\n'"${PROPS_LINES}"
fi

