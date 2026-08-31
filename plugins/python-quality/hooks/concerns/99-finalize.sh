# --- Output ---

if [[ -n "$MESSAGES" ]]; then
  # Record this file's violation in the gate status file (entry keyed by file
  # path — does not clobber failures recorded for other files or hooks).
  GATE_DIR="$(toolu_project_state_root "$PROJECT_ROOT")"
  GATE_FILE="$GATE_DIR/quality-gate-status.json"
  mkdir -p "$GATE_DIR"
  gate_record_failure "$GATE_FILE" "$FILE_PATH" "python-quality-hook" \
    "Post-edit Python quality violation(s) detected" "$MESSAGES"

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
  GATE_FILE="$(toolu_project_state_root "$PROJECT_ROOT")/quality-gate-status.json"
  gate_clear_file "$GATE_FILE" "$FILE_PATH" "python-quality-hook"

  # Docs advisory (non-blocking — no gate write).
  if [[ -n "$DOC_ADVISORY" ]]; then
    jq -n --arg ctx "$DOC_ADVISORY" '{
      "hookSpecificOutput": {
        "hookEventName": "PostToolUse",
        "additionalContext": $ctx
      }
    }'
  fi
fi

exit 0
