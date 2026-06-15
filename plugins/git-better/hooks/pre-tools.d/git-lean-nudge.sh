#!/bin/bash
# Pre-tool nudge (Pillar 1): steer token-heavy RAW git reads toward `gb` lean
# forms. Advisory only — never blocks, never rewrites. Fires on bare
# status/diff/log/show lacking a lean flag; silent on lean flags, write/workflow
# verbs, and output already piped to a limiter.
#
# Inputs (exported by the toolu pre-tools dispatcher): $tool_name $input
: "${tool_name:=}"
: "${input:=}"

# Fail SOFT: this is an advisory extra; without the toolu lib it must no-op,
# never error a tool call.
[ -n "${TOOLU_LIB_DIR:-}" ] && [ -f "$TOOLU_LIB_DIR/detect.sh" ] && [ -f "$TOOLU_LIB_DIR/config.sh" ] || exit 0
# shellcheck source=../../../toolu/hooks/lib/detect.sh
. "$TOOLU_LIB_DIR/detect.sh"
# shellcheck source=../../../toolu/hooks/lib/config.sh
. "$TOOLU_LIB_DIR/config.sh"

{ [ "$tool_name" = "Bash" ] || [ "$tool_name" = "Shell" ]; } || exit 0
command -v jq >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')
[ -n "$command" ] || exit 0
cmd_only=$(printf '%s\n' "$command" | strip_heredocs)

# User opted out of git-better → no nudge.
toolu_enabled skills git-better || exit 0

# Must contain a git invocation at all.
echo "$cmd_only" | grep -qE '\bgit\b' || exit 0

# Already bounded (piped to a limiter or redirected to a file) → silent.
echo "$cmd_only" | grep -qE '\|[[:space:]]*(head|tail|wc|less|more|sed[[:space:]]+-[En]*n)\b' && exit 0
echo "$cmd_only" | grep -qE '>[[:space:]]*[^&[:space:]]' && exit 0

emit() {
  jq -n --arg c "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:$c}}'
  exit 0
}

# status — lean: -s/-sb/--short/--porcelain
if echo "$cmd_only" | grep -qE '\bgit\b[^|]*\bstatus\b'; then
  echo "$cmd_only" | grep -qE '(-sb|-s\b|--short|--porcelain)' \
    || emit "git status is verbose. Use \`gb status\` (git status -sb, no color) for a compact view."
fi

# diff — lean/targeted: stat family, -U<n>, an explicit `-- `, or a path
# (slash path OR a flat dotted filename like `README.md`).
if echo "$cmd_only" | grep -qE '\bgit\b[^|]*\bdiff\b'; then
  echo "$cmd_only" | grep -qE '(--stat|--numstat|--name-only|--name-status|--shortstat|--compact-summary|-U[0-9]|--unified| -- |/|\.[A-Za-z0-9]+)' \
    || emit "git diff dumps full hunks (incl. lockfiles). Use \`gb diff\` (stat-first, lockfiles excluded), then \`gb diff <path>\` to drill in, or \`gb diff --cached\`."
fi

# log — lean: oneline/format/pretty/stat or a count limit
if echo "$cmd_only" | grep -qE '\bgit\b[^|]*\blog\b'; then
  echo "$cmd_only" | grep -qE '(--oneline|--format|--pretty|--stat|--shortstat|-n[ =]|-[0-9])' \
    || emit "git log is verbose. Use \`gb log\` (git log --oneline -n 20); pass -n/--format to override."
fi

# show — lean/targeted: stat family or a path (slash or flat dotted filename)
if echo "$cmd_only" | grep -qE '\bgit\b[^|]*\bshow\b'; then
  echo "$cmd_only" | grep -qE '(--stat|--numstat|--name-only|--name-status|--shortstat| -- |/|\.[A-Za-z0-9]+)' \
    || emit "git show dumps the full diff. Use \`gb show\` (show --stat HEAD); pass a ref/path to forward."
fi

exit 0
