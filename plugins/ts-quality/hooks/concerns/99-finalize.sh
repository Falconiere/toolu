# --- Output ---

if [[ -n "$MESSAGES" ]]; then
  # Record this file's violation in the gate status file (entry keyed by file
  # path — does not clobber failures recorded for other files or hooks).
  GATE_DIR="$PROJECT_ROOT/.claude/tmp"
  GATE_FILE="$GATE_DIR/quality-gate-status.json"
  mkdir -p "$GATE_DIR"
  gate_record_failure "$GATE_FILE" "$FILE_PATH" "ts-quality-hook" \
    "Post-edit quality violation(s) detected" "$MESSAGES"

  jq -n --arg ctx "$MESSAGES" '{
    "hookSpecificOutput": {
      "hookEventName": "PostToolUse",
      "additionalContext": ("QUALITY VIOLATION — fix before proceeding:\n" + $ctx)
    }
  }'
else
  # Clear this file's entry now that it passes (only if this hook set it).
  # Other files' failures stay recorded; the gate only flips to passing when
  # no entry remains.
  GATE_FILE="$PROJECT_ROOT/.claude/tmp/quality-gate-status.json"
  gate_clear_file "$GATE_FILE" "$FILE_PATH" "ts-quality-hook"

  # Advisories (non-blocking — no gate write). Duplication + docs combined into
  # a single additionalContext so the hook emits exactly one JSON object.
  # Joined with REAL newlines. `${ADVISORY:+$ADVISORY\n}` appended a literal
  # backslash-n (and ANSI-C quoting is not honored inside a :+ expansion either),
  # so stacked advisories reached the agent as one run-on line.
  ADVISORY="$DUPLICATION_WARNING"
  for _adv in "$DOC_ADVISORY" "$ERR_ADVISORY"; do
    [[ -z "$_adv" ]] && continue
    [[ -n "$ADVISORY" ]] && ADVISORY="${ADVISORY}"$'\n'
    ADVISORY="${ADVISORY}${_adv}"
  done
  if [[ -n "$ADVISORY" ]]; then
    jq -n --arg ctx "$ADVISORY" '{
      "hookSpecificOutput": {
        "hookEventName": "PostToolUse",
        "additionalContext": $ctx
      }
    }'
  fi
fi

exit 0
