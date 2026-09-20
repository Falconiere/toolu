#!/usr/bin/env bash
# UserPromptSubmit hook — restate the Jev mandate on every task.
#
# SessionStart states the full rule once per startup, resume, clear, and
# compaction, but a long session drifts away from it. Each user prompt is a
# task, so each one gets a short reminder naming the published wrapper. No
# network calls; the hook only checks local prerequisites, exactly like
# SessionStart.
#
# Silent cases, deliberately:
#   - trivial confirmations (y / ok / lgtm ...) are not tasks;
#   - missing prerequisites — SessionStart already reported them, repeating
#     the same failure on every prompt is noise the agent cannot act on.
set -euo pipefail

HOOK_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
# Fail soft on a corrupted install: a missing lib must never break the session.
[ -f "$HOOK_DIR/lib/common.sh" ] || exit 0
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

input=$(cat 2>/dev/null || true)
prompt=""
if command -v jq >/dev/null 2>&1; then
  prompt=$(jq -r '.prompt // ""' <<<"$input" 2>/dev/null || true)
fi
[ -n "$prompt" ] || exit 0

prompt_lower=$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')
if [[ "$prompt_lower" =~ ^[[:space:]]*(y|n|yes|no|ok|okay|sure|thanks|thank\ you|go\ ahead|looks\ good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[\.\!\?]?[[:space:]]*$ ]]; then
  exit 0
fi

plugin_dir="$(cd "$HOOK_DIR/.." 2>/dev/null && pwd)"
dst="$(jev_published_path)"
[ -z "$(jev_missing_prereqs "$dst")" ] || exit 0

context="Jev is mandatory for this task: before acting, you MUST call \"$dst\" on at least one bounded semantic decision the request contains (classify the request or its scope, rank or route candidate approaches, judge supplied evidence), batching independent questions in one ask call. If the task has no semantic decision, say so in one sentence. Syntax: $plugin_dir/skills/jev/SKILL.md."
jev_emit UserPromptSubmit "$context"

exit 0
