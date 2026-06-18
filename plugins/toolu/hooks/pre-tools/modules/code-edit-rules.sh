#!/usr/bin/env bash
# Pre-tool check: Remind to read rules before editing code files.
# Data-driven: loads $TOOLU_SETTINGS_DIR/code-edit-rules.json. Empty/missing → no-op.
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
# like Edit/Write — include it so rule reminders are not dropped for MultiEdit.
[[ "$tool_name" != "Edit" && "$tool_name" != "Write" && "$tool_name" != "MultiEdit" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
RULES_FILE="$TOOLU_SETTINGS_DIR/code-edit-rules.json"

[ -f "$RULES_FILE" ] || exit 0

file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""')
[ -z "$file_path" ] && exit 0

# Normalize abs → repo-relative so patterns in code-edit-rules.json match
# the same way the user wrote them.
file_path=$(to_relative_path "$file_path")

# glob_match <pattern> <path> — bash-native glob check
glob_match() {
  local pattern="$1"
  local path="$2"
  shopt -s extglob globstar 2>/dev/null || true
  # shellcheck disable=SC2053  # intentional glob match on RHS
  [[ "$path" == $pattern ]]
}

# Iterate rules and accumulate docs whose match-glob fits.
docs=""
rule_count=$(jq '.rules | length' "$RULES_FILE" 2>/dev/null || echo 0)
[ "$rule_count" = "null" ] && rule_count=0

for ((i=0; i<rule_count; i++)); do
  match=$(jq -r ".rules[$i].match // \"\"" "$RULES_FILE")
  [ -z "$match" ] && continue
  if glob_match "$match" "$file_path"; then
    # Base docs
    base=$(jq -r ".rules[$i].docs // [] | join(\" + \")" "$RULES_FILE")
    [ -n "$base" ] && [ "$base" != "null" ] && docs="${docs}${docs:+ + }${base}"
    # Conditional extra docs
    extra_paths_count=$(jq -r ".rules[$i].when_path_matches // [] | length" "$RULES_FILE")
    if [ "$extra_paths_count" -gt 0 ]; then
      for ((j=0; j<extra_paths_count; j++)); do
        cond=$(jq -r ".rules[$i].when_path_matches[$j]" "$RULES_FILE")
        if glob_match "$cond" "$file_path"; then
          extra=$(jq -r ".rules[$i].extra_docs // [] | join(\" + \")" "$RULES_FILE")
          [ -n "$extra" ] && [ "$extra" != "null" ] && docs="${docs}${docs:+ + }${extra}"
          break
        fi
      done
    fi
    break
  fi
done

[ -z "$docs" ] && exit 0

jq -n --arg msg "Apply these rules: $docs" --arg file "$file_path" '{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "additionalContext": ("File: " + $file + "\n" + $msg)
  }
}'
exit 0
