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

context="Jev is mandatory for this task when it contains semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call \"$dst\" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: $plugin_dir/skills/jev/SKILL.md. On failure, state the limitation and use an evidence fallback; Jev never replaces tests or authorization."
jev_emit UserPromptSubmit "$context"

exit 0
