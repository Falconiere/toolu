#!/bin/bash
# Pre-tool nudge (Pillar 2): when about to commit or open a PR, remind Claude to
# match house conventions via `gb conventions`. Advisory only. Includes the
# cached one-line profile summary when a profile already exists for the repo, so
# the reminder is actionable without a tool round-trip.
#
# Inputs (exported by the toolu pre-tools dispatcher): $tool_name $input
: "${tool_name:=}"
: "${input:=}"

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

toolu_enabled skills git-better || exit 0

# Fire only when committing or opening a PR.
echo "$cmd_only" | grep -qE '(\bgit\b[^|]*\bcommit\b|\bgh\b[^|]*\bpr\b[^|]*\bcreate\b)' || exit 0

_sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi; }

# Best-effort: read the cached profile summary for this repo.
summary=""
root=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [ -n "$root" ]; then
  cfg="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}"
  cf="$cfg/toolu/git-better/conventions/$(printf '%s' "$root" | _sha256).json"
  if [ -f "$cf" ]; then
    summary=$(jq -r '
      "commit \(.commit_format.convention)"
      + (if .commit_format.scope=="used" then "+scope" else "" end)
      + (if .commit_format.pr_suffix then "+\(.commit_format.pr_suffix)" else "" end)
      + "; branch \(.branch_naming.pattern)"' "$cf" 2>/dev/null || true)
  fi
fi

msg="Committing / opening a PR — match house conventions first: run \`gb conventions\`."
[ -n "$summary" ] && msg="$msg This repo: $summary."

jq -n --arg c "$msg" '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:$c}}'
exit 0
