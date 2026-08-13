#!/usr/bin/env bash
# PostToolUse instrumentation — measure how many bytes targeted/structural tools
# inject into context, so the "ast-grep returns far less than reading whole
# files" claim is MEASURED, not asserted.
#
# Honest by construction: it records the REAL byte count each tool returned. For
# a Read of a single on-disk file it also records that file's full size (the
# bytes a whole read costs), so a ranged Read shows a real saving. It does NOT
# fabricate a counterfactual for searches — it records what ast-grep/Grep
# actually returned, and the report aggregates per tool so the volume gap is
# visible. Append-only ledger; never blocks; emits nothing to context.
#
# Inputs (exported by post-tools/mod.sh): $tool_name $input $PROJECT_ROOT
: "${tool_name:=}"
: "${input:=}"

command -v jq >/dev/null 2>&1 || exit 0

# Classify the tool; ignore everything we don't measure as cheaply as possible.
kind=""
case "$tool_name" in
  Read) kind="read" ;;
  Grep) kind="grep" ;;
  Glob) kind="glob" ;;
  Bash|Shell)
    cmd=$(jq -r '.tool_input.command // ""' <<<"$input" 2>/dev/null)
    # Matches `ast-grep …`, `sg …`, and piped forms (`… | ast-grep …`) — the
    # leading-space alternative subsumes the pipe case, so no separate pattern.
    case "$cmd" in
      ast-grep*|sg\ *|*\ ast-grep\ *) kind="ast-grep" ;;
      *) exit 0 ;;
    esac
    ;;
  *) exit 0 ;;
esac

# Returned bytes = byte length of the tool's textual response. tool_response may
# be a string (Read/Grep) or an object (Bash); coerce to text then measure.
resp=$(jq -r '
  .tool_response
  | if   type == "string" then .
    elif type == "object" then (.content? // .stdout? // .output? // tostring)
    else tostring end' <<<"$input" 2>/dev/null)
[ -n "$resp" ] || exit 0
returned=$(printf '%s' "$resp" | wc -c | tr -d ' ')
[ -n "$returned" ] || exit 0

# Full-file bytes (single-file Read only): the cost a whole read would incur.
full=0
if [ "$kind" = "read" ]; then
  fp=$(jq -r '.tool_input.file_path // ""' <<<"$input" 2>/dev/null)
  if [ -n "$fp" ] && [ -f "$fp" ]; then
    full=$(wc -c < "$fp" 2>/dev/null | tr -d ' ')
    [ -n "$full" ] || full=0
  fi
fi

# Append one compact record to the per-session ledger. Session-scoped so a
# stale id can never grow unbounded; the report aggregates a single session.
sid=$(jq -r '.session_id // "unknown"' <<<"$input" 2>/dev/null | tr -cd 'A-Za-z0-9-')
[ -n "$sid" ] || sid="unknown"
ledger_dir="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}/toolu/byte-savings"
mkdir -p "$ledger_dir" 2>/dev/null || exit 0
printf '{"kind":"%s","returned":%s,"full":%s}\n' "$kind" "$returned" "$full" \
  >> "$ledger_dir/$sid.jsonl" 2>/dev/null

exit 0
