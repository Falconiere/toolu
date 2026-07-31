#!/usr/bin/env bash
# Agent/Task model-tier telemetry + advisory. Standalone PreToolUse hook — NOT
# under pre-tools/modules/ (mod.sh only dispatches that directory, so a
# sibling script here is invisible to it and, like mcp-blocker.sh, must read
# its own stdin instead of relying on the parent dispatcher's `export`s).
#
# Registered in hooks.json under matcher "Agent|Task" — deliberately loose
# (it also matches TaskCreate/TaskUpdate/etc.); the real guard is the exact
# tool_name equality check below (resolves spec OQ-1).
#
# Behavior:
#   - Anything but tool_name == "Agent" or "Task" -> exit 0, no output.
#   - Always append a `delegation` telemetry event: {model, subagent_type,
#     step_id, step_model}, all null when not applicable. The plan-ledger join
#     (step_id/step_model) only happens when a ledger file for the current
#     branch exists (cheap stat first): step_id is the running step's id, else
#     the ledger's `next`; step_model is that step's declared `model` field.
#   - Nudge/deny ONLY when the joined step names a non-null model AND the
#     call supplied a `model` param AND they differ. A missing call `model`
#     means tier-inherit and is always legitimate — never nudged, never
#     denied. Controlled by `agentTier.mode`: advise (default, emits
#     additionalContext) | block (deny) | off (silent).
#   - All failure paths (no jq, unreadable/malformed ledger, no git repo)
#     fail open (exit 0); a telemetry bug must never block a delegation.

set -o pipefail

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../lib}"
# shellcheck source=../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../lib/config.sh
. "$_toolu_lib/config.sh"
# shellcheck source=../lib/telemetry.sh
. "$_toolu_lib/telemetry.sh"

command -v jq >/dev/null 2>&1 || exit 0

_input=$(cat 2>/dev/null || true)

tool_name=$(jq -r '.tool_name // empty' <<<"$_input" 2>/dev/null || true)

# Exact-equality self-filter: the hooks.json matcher is deliberately loose
# (Agent|Task), so everything else — including TaskCreate/TaskUpdate — must be
# silently ignored here.
case "$tool_name" in
  Agent|Task) ;;
  *) exit 0 ;;
esac

command -v git >/dev/null 2>&1 || exit 0

root=$(detect_project_root)
[ -n "$root" ] || exit 0

call_model=$(jq -r '.tool_input.model // empty' <<<"$_input" 2>/dev/null || true)
subagent_type=$(jq -r '.tool_input.subagent_type // empty' <<<"$_input" 2>/dev/null || true)

# Ledger join: cheap stat first, so a repo with no active plan pays no extra
# jq cost. LEDGER_DIR mirrors pre-tools/modules/plan-ledger.sh's override.
step_id="" step_model=""
branch=$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
if [ -n "$branch" ] && [ "$branch" != "HEAD" ]; then
  slug=$(branch_slug "$branch")
  ledger_dir="${LEDGER_DIR:-$root/.claude/tmp/plan-ledger}"
  ledger_file="$ledger_dir/${slug}.json"
  if [ -f "$ledger_file" ]; then
    ledger_json=$(cat "$ledger_file" 2>/dev/null || true)
    if [ -n "$ledger_json" ] && jq -e . >/dev/null 2>&1 <<<"$ledger_json"; then
      # Running step's id if one is running, else the ledger's `next`
      # (already null when every step is fresh-green).
      step_id=$(jq -r '
        (first(.steps[]? | select(.status == "running") | .id)) // (.next // empty)
      ' <<<"$ledger_json" 2>/dev/null || true)
      if [ -n "$step_id" ]; then
        step_model=$(jq -r --arg id "$step_id" \
          '(first(.steps[]? | select(.id == $id) | .model)) // empty' \
          <<<"$ledger_json" 2>/dev/null || true)
      fi
    fi
  fi
fi

# Always record the delegation — join or not, nudge or not.
extra=$(jq -n \
  --arg model "$call_model" \
  --arg subagent_type "$subagent_type" \
  --arg step_id "$step_id" \
  --arg step_model "$step_model" \
  '{
    model:         (if $model == ""         then null else $model end),
    subagent_type: (if $subagent_type == "" then null else $subagent_type end),
    step_id:       (if $step_id == ""       then null else $step_id end),
    step_model:    (if $step_model == ""    then null else $step_model end)
  }')
telemetry_append "$root" delegation "$extra"

# Nudge/deny: only when the joined step declares a non-null model AND the
# call supplied one AND they differ. A missing call model is tier-inherit —
# always legitimate, never nudged or denied.
if [ -n "$step_model" ] && [ -n "$call_model" ] && [ "$step_model" != "$call_model" ]; then
  mode=$(toolu_string agentTier.mode advise advise block off)
  reason="plan step \"$step_id\" expects model tier \"$step_model\" but this delegation used \"$call_model\""
  case "$mode" in
    advise)
      jq -n --arg reason "$reason" '{
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          additionalContext: $reason
        }
      }'
      ;;
    block)
      jq -n --arg reason "$reason" '{
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          permissionDecision: "deny",
          permissionDecisionReason: $reason
        }
      }'
      ;;
    off) ;;
  esac
fi

exit 0
