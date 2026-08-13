#!/usr/bin/env bash
# Print the explicit status report used by the Codex status skill.
set -euo pipefail

SCRIPT_DIR=$(cd "${BASH_SOURCE[0]%/*}" && pwd)
status_json=$(TOOLU_HOST_OVERRIDE=codex \
  bash "$SCRIPT_DIR/collect-status.sh" "${1:-$PWD}")

host=$(jq -r '.host' <<<"$status_json")
repo_root=$(jq -r '.repo_root' <<<"$status_json")
folder=$(jq -r '.folder' <<<"$status_json")
branch=$(jq -r '.branch' <<<"$status_json")
ahead=$(jq -r '.ahead' <<<"$status_json")
behind=$(jq -r '.behind' <<<"$status_json")
staged=$(jq -r '.working_tree.staged' <<<"$status_json")
unstaged=$(jq -r '.working_tree.unstaged' <<<"$status_json")
untracked=$(jq -r '.working_tree.untracked' <<<"$status_json")
gate_status=$(jq -r '.gate.status' <<<"$status_json")
gate_reason=$(jq -r '.gate.reason' <<<"$status_json")
comemory=$(jq -r '.comemory_count // empty' <<<"$status_json")

[ "$host" = codex ] && host_label=Codex || host_label='Claude Code'
printf 'Host: %s\n' "$host_label"
if [ -n "$repo_root" ]; then
  printf 'Repository: %s\n' "$repo_root"
  [ -n "$branch" ] && printf 'Branch: %s' "$branch" || printf 'Branch: detached HEAD'
  [ "$ahead" -gt 0 ] && printf ' (ahead %s)' "$ahead"
  [ "$behind" -gt 0 ] && printf ' (behind %s)' "$behind"
  printf '\n'
  if [ "$staged" -eq 0 ] && [ "$unstaged" -eq 0 ] && [ "$untracked" -eq 0 ]; then
    printf 'Working tree: clean\n'
  else
    printf 'Working tree: staged %s, unstaged %s, untracked %s\n' \
      "$staged" "$unstaged" "$untracked"
  fi
else
  printf 'Folder: %s (not a git repository)\n' "$folder"
fi

case "$gate_status" in
  failing)
    printf 'Quality gate: failing'
    [ -z "$gate_reason" ] || printf ' — %s' "$gate_reason"
    printf '\n'
    ;;
  passing) printf 'Quality gate: passing\n' ;;
  *) printf 'Quality gate: no recorded state\n' ;;
esac
[ -z "$comemory" ] || printf 'Comemory: %s memories\n' "$comemory"
