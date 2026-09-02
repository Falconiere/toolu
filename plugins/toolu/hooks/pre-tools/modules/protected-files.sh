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
#
# Bash/Shell are also in scope: a structured Edit/Write on a protected path is
# denied, but the same bytes written via `sed -i`, a redirect, or `python3 -c
# "open(...).write(...)"` had no path-shaped tool_input to check and sailed
# through — the exact gap reported in
# github.com/Falconiere/toolu/issues/176. bash_write_targets (lib/detect.sh)
# extracts candidate write targets from the command; each is checked against
# the same protected-files.txt patterns as Edit/Write.
[[ "$tool_name" != "Edit" && "$tool_name" != "Write" && "$tool_name" != "MultiEdit" \
  && "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
LIST_FILE="$TOOLU_SETTINGS_DIR/protected-files.txt"

[ -f "$LIST_FILE" ] || exit 0

declare -a candidates=()
if [[ "$tool_name" == "Bash" || "$tool_name" == "Shell" ]]; then
  command_str=$(echo "$input" | jq -r '.tool_input.command // ""')
  [ -z "$command_str" ] && exit 0
  while IFS= read -r target; do
    [ -n "$target" ] && candidates+=("$target")
  done < <(bash_write_targets "$command_str")
  [ ${#candidates[@]} -eq 0 ] && exit 0
else
  file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""')
  [ -z "$file_path" ] && exit 0
  candidates=("$file_path")
fi

# read_list is sourced from lib/detect.sh.

# glob_match <pattern> <path>
glob_match() {
  local pattern="$1"
  local path="$2"
  # Enable extended globs for ** to match path segments.
  shopt -s extglob globstar 2>/dev/null || true
  # shellcheck disable=SC2053  # intentional glob match on RHS
  [[ "$path" == $pattern ]]
}

list=$(read_list "$LIST_FILE")

# Normalize absolute paths (Edit/Write sends absolute; a Bash write target is
# whatever the command wrote literally, often already relative) into
# repo-relative so patterns like "hooks/lib/**" can match. Falls back to the
# input unchanged outside a git repo (test sandboxes etc).
for candidate in "${candidates[@]}"; do
  rel_path=$(to_relative_path "$candidate")
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
  done <<< "$list"

  if [ -n "$matched" ]; then
    if [[ "$tool_name" == "Bash" || "$tool_name" == "Shell" ]]; then
      jq -n --arg p "$matched" --arg f "$candidate" '{
        "hookSpecificOutput": {
          "hookEventName": "PreToolUse",
          "permissionDecision": "deny",
          "permissionDecisionReason": ("Command would write to " + $f + ", which is protected (matches \"" + $p + "\"). Same guardrail as Edit/Write: it always denies, in every tool, with no session override (see plugins/toolu/hooks/docs/gates.md).")
        }
      }'
    else
      jq -n --arg p "$matched" --arg f "$candidate" '{
        "hookSpecificOutput": {
          "hookEventName": "PreToolUse",
          "permissionDecision": "deny",
          "permissionDecisionReason": ("File " + $f + " is protected (matches \"" + $p + "\"). This guardrail always denies: it is not a judgement call a user approval can relax (see plugins/toolu/hooks/docs/gates.md).")
        }
      }'
    fi
    exit 0
  fi
done

exit 0
