#!/usr/bin/env bash
# Pre-tool check: Protect quality infrastructure files from edits.
# Data-driven: globs come from $TOOLU_SETTINGS_DIR/protected-files.txt.
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"

# MultiEdit is in the PreToolUse matcher and carries .tool_input.file_path just
# like Edit/Write — omitting it here would let a MultiEdit silently bypass the
# protected-files deny (a security-equivalent hole).
[[ "$tool_name" != "Edit" && "$tool_name" != "Write" && "$tool_name" != "MultiEdit" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
LIST_FILE="$TOOLU_SETTINGS_DIR/protected-files.txt"

[ -f "$LIST_FILE" ] || exit 0

file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""')
[ -z "$file_path" ] && exit 0

# read_list is sourced from lib/detect.sh.

# Normalize absolute paths (Edit/Write sends absolute) into repo-relative so
# patterns like "hooks/lib/**" can match. Falls back to the input unchanged
# outside a git repo (test sandboxes etc).
rel_path=$(to_relative_path "$file_path")

# glob_match <pattern> <path>
glob_match() {
  local pattern="$1"
  local path="$2"
  # Enable extended globs for ** to match path segments.
  shopt -s extglob globstar 2>/dev/null || true
  # shellcheck disable=SC2053  # intentional glob match on RHS
  [[ "$path" == $pattern ]]
}

matched=""
while IFS= read -r pattern; do
  [ -z "$pattern" ] && continue
  # Try as-is.
  if glob_match "$pattern" "$rel_path"; then
    matched="$pattern"
    break
  fi
  # For patterns that do not already start with **/, also try anchored
  # anywhere under the repo. This keeps "hooks/lib/**" matching even when
  # the path arrives as e.g. "subtree/hooks/lib/x.sh".
  if [[ "$pattern" != \*\*/* ]] && glob_match "**/$pattern" "$rel_path"; then
    matched="$pattern"
    break
  fi
  # Also check basename for patterns without a path separator.
  if [[ "$pattern" != */* ]]; then
    base=$(basename "$rel_path")
    if glob_match "$pattern" "$base"; then
      matched="$pattern"
      break
    fi
  fi
done <<< "$(read_list "$LIST_FILE")"

if [ -n "$matched" ]; then
  jq -n --arg p "$matched" --arg f "$file_path" '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "deny",
      "permissionDecisionReason": ("File " + $f + " is protected (matches \"" + $p + "\"). Editing is not allowed unless explicitly requested by the user.")
    }
  }'
  exit 0
fi

exit 0
