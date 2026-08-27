#!/usr/bin/env bash
# Pre-commit gate — validates Conventional Commits prefix and reminds about
# scope verification + memory save before commits.
# Project-agnostic: prefixes come from settings/commit-prefixes.txt; base
# branch is detected via detect_base_branch.

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/gate-mode.sh
. "$_toolu_lib/gate-mode.sh"

[[ "$tool_name" != "Bash" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null || echo "")

# Only match git commit commands. is_git_commit (lib/detect.sh) is used rather
# than a literal prefix match so `git -C <path> commit` and a commit reached
# through `&&` are seen too, and a heredoc body that merely mentions committing
# is not.
is_git_commit "$command" || exit 0

mode=$(toolu_gate_mode commitGate)
[ "$mode" = "off" ] && exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
PREFIX_FILE="$TOOLU_SETTINGS_DIR/commit-prefixes.txt"
BASE_BRANCH=$(detect_base_branch)

# read_list is sourced from lib/detect.sh.

# Extract the message from -m "..." if present; else allow (it's editor-driven).
# Double-quoted form honors backslash escapes (`-m "fix \"bug\""`) so an
# escaped quote no longer truncates the message; single-quoted form cannot
# contain escapes by shell rules. If neither form parses, msg stays empty and
# the gate falls through gracefully (no deny, context message only).
msg=""
if [[ "$command" == *" -m "* ]]; then
  msg=$(printf '%s' "$command" | sed -nE 's/.* -m[[:space:]]+"((\\.|[^"\\])*)".*/\1/p')
  if [ -z "$msg" ]; then
    msg=$(printf '%s' "$command" | sed -nE "s/.* -m[[:space:]]+'([^']*)'.*/\\1/p")
  else
    # Unescape so the subject parser sees the literal message (\" -> ").
    msg=$(printf '%s' "$msg" | sed -E 's/\\(.)/\1/g')
  fi
fi

prefixes=$(read_list "$PREFIX_FILE")
if [ -n "$msg" ] && [ -n "$prefixes" ]; then
  # Subject = first line up to first colon/space
  subject_prefix=$(printf '%s' "$msg" | head -1 | sed -nE 's/^([a-z]+)(\(.*\))?:.*/\1/p')
  if [ -n "$subject_prefix" ]; then
    if ! echo "$prefixes" | grep -qFx "$subject_prefix"; then
      toolu_gate_emit "$mode" "Unknown Conventional Commits prefix: \"$subject_prefix\". Allowed prefixes are in settings/commit-prefixes.txt. Base branch: $BASE_BRANCH"
      exit 0
    fi
  fi
fi

jq -n --arg base "$BASE_BRANCH" '{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "additionalContext": ("BEFORE COMMITTING:\n1. Verify diff covers only expected scope (git diff --stat against " + $base + ")\n2. Save memory of significant decisions before committing.\nSkip only if already done this task.")
  }
}'
exit 0
