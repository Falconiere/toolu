AS_LINES=$(grep -nE '\)[[:space:]]+as[[:space:]]+[a-zA-Z]|\bas[[:space:]]+any\b|\bas[[:space:]]+unknown\b|[a-zA-Z>][[:space:]]+as[[:space:]]+[A-Z]|[a-zA-Z>][[:space:]]+as[[:space:]]+(string|number|boolean|object|symbol|bigint|never|undefined|null|void)\b' "$FILE_PATH" 2>/dev/null \
  | grep -vE '^[0-9]+:[[:space:]]*//' \
  | grep -vE '\bas[[:space:]]+const\b' \
  | grep -vE '\bimport\b|^[0-9]+:[[:space:]]*export[[:space:]]*(type[[:space:]]+)?\{' \
  | head -5)
if [[ -n "$AS_LINES" ]]; then
  add_error "Forbidden 'as' type assertion in $FILE_PATH — use type guards or Zod"$'\n'"${AS_LINES}"
fi

