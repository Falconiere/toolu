#!/usr/bin/env bash
# Pre-tool check: quality gate enforcement.
#
# When the gate state file says "failing", this stops the two actions that
# would let a red build escape the working tree: `git commit` and `git push`.
# Nothing else. Reading, searching, editing, and running commands stay open —
# a failing gate is a reason not to SHIP, not a reason to be unable to work.
# (It used to deny every tool call except a whitelist, which meant a stale red
# gate could block `ls`.)
#
# How firmly it stops them is a mode — see lib/gate-mode.sh. Default: block.
# `MY_CLAUDE_QUALITY=off` remains an env-level kill switch for the whole gate.
#
# Project-agnostic: tooling is auto-detected; missing tooling → silent skip.
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/gate-mode.sh
. "$_toolu_lib/gate-mode.sh"

# Owner kill-switch.
[ "${MY_CLAUDE_QUALITY:-}" = "off" ] && exit 0

# Guard tooling.
command -v jq  >/dev/null 2>&1 || exit 0
command -v git >/dev/null 2>&1 || exit 0

# Only Bash/Shell can commit or push, so every other tool is out of scope
# before any file is read.
[[ "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')
cmd_only=$(printf '%s\n' "$command" | strip_heredocs)

# Heredoc-stripped, boundary-anchored detection shared with push-review, so
# `git -C <path> commit` and `foo && git push` are seen and a commit message
# that merely mentions pushing is not.
if ! is_git_commit "$cmd_only" && ! is_git_push "$cmd_only"; then
  exit 0
fi

# Only now is the config worth loading. This module runs on every Bash call and
# almost none of them are a commit or a push; resolving the mode first would
# have paid a config read + jq parse for every `ls`.
mode=$(toolu_gate_mode qualityGate)
[ "$mode" = "off" ] && exit 0

project_root="$(detect_project_root)"
[ -z "$project_root" ] && project_root="$(pwd)"

gate_file="$(toolu_project_state_root "$project_root")/quality-gate-status.json"

[[ ! -f "$gate_file" ]] && exit 0
[[ "$(jq -r '.status // ""' "$gate_file" 2>/dev/null)" != "failing" ]] && exit 0

# Skip enforcement in git linked worktrees — quality state lives on the main checkout.
git_dir="$(git -C "$project_root" rev-parse --path-format=absolute --git-dir 2>/dev/null || true)"
common_dir="$(git -C "$project_root" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
git_dir="${git_dir%/}"
common_dir="${common_dir%/}"
if [[ -n "$git_dir" && -n "$common_dir" && "$git_dir" != "$common_dir" ]]; then
  exit 0
fi

reason=$(jq -r '.reason // "Quality gate failing"' "$gate_file" 2>/dev/null || echo "Quality gate failing")
violations=$(jq -r '.violations // ""' "$gate_file" 2>/dev/null || echo "")

# The lead sentence follows the mode: claiming something was BLOCKED while
# merely advising would be a lie in the one place the user is reading closely.
case "$mode" in
  block) lead="BLOCKED: quality gate failing — fix the violations before committing or pushing." ;;
  ask)   lead="The quality gate is failing. Commit/push anyway?" ;;
  *)     lead="Heads up: the quality gate is failing (commit and push are not blocked at this setting)." ;;
esac

toolu_gate_emit "$mode" "$lead
$reason
$violations"
exit 0
