#!/bin/bash
# PreCompact hook

HOOK_DIR="$(dirname "$0")"

# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

if ! toolu_enabled hooks pre-compact; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

# Consume stdin (Claude Code sends hook input via stdin)
cat > /dev/null 2>&1 || true

reminder="Context compacted. Continue from the available session context."

jq -n --arg reminder "$reminder" '{
  "hookSpecificOutput": {
    "hookEventName": "PreCompact",
    "additionalContext": $reminder
  }
}'
