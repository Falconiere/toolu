#!/usr/bin/env bash
# PostToolUse instrumentation — record how many bytes git reads inject into
# context, split by kind so the "gb is leaner than raw git" claim is MEASURED,
# not asserted. Honest by construction: records the REAL returned byte count; it
# fabricates no counterfactual (full=0). Append-only ledger; never blocks; emits
# nothing to context. Shares the toolu byte-savings ledger with other plugins.
#
# Inputs (exported by post-tools/mod.sh): $tool_name $input
: "${tool_name:=}"
: "${input:=}"

command -v jq >/dev/null 2>&1 || exit 0
{ [ "$tool_name" = "Bash" ] || [ "$tool_name" = "Shell" ]; } || exit 0

cmd=$(jq -r '.tool_input.command // ""' <<<"$input" 2>/dev/null)
[ -n "$cmd" ] || exit 0

# Classify: a gb wrapper call (shim `gb` or the script) vs a bare raw git read.
kind=""
if printf '%s' "$cmd" | grep -qE '(^|[[:space:]/])gb[[:space:]]|git-better\.sh'; then
  kind="gb"
elif printf '%s' "$cmd" | grep -qE '\bgit\b[^|]*\b(status|diff|log|show)\b'; then
  kind="git-raw"
else
  exit 0
fi

# Returned bytes = byte length of the tool's textual response. tool_response may
# be a string (simple) or an object (Bash); coerce to text then measure.
resp=$(jq -r '
  .tool_response
  | if   type == "string" then .
    elif type == "object" then (.content? // .stdout? // .output? // tostring)
    else tostring end' <<<"$input" 2>/dev/null)
[ -n "$resp" ] || exit 0
returned=$(printf '%s' "$resp" | wc -c | tr -d ' ')
[ -n "$returned" ] || exit 0

sid=$(jq -r '.session_id // "unknown"' <<<"$input" 2>/dev/null | tr -cd 'A-Za-z0-9-')
[ -n "$sid" ] || sid="unknown"
ledger_dir="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}/toolu/byte-savings"
mkdir -p "$ledger_dir" 2>/dev/null || exit 0
printf '{"kind":"%s","returned":%s,"full":0}\n' "$kind" "$returned" \
  >> "$ledger_dir/$sid.jsonl" 2>/dev/null

exit 0
