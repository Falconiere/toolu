#!/usr/bin/env bash
# SessionStart hook — publish the jev wrapper at a STABLE, env-independent
# path. See exa-search's session-start.sh for the same fix rationale:
# ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only — NOT to the
# agent's Bash tool subshell — so a SKILL.md path of
# "${CLAUDE_PLUGIN_ROOT}/skills/jev/scripts/jev.sh" expands to
# "/skills/.../jev.sh: No such file" when an agent pastes it. Symlink the
# wrapper to ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/jev/jev.sh instead.
# Refreshed every session; inject the mandate without network calls.
set -euo pipefail

# Consume stdin so Claude Code's hook IPC never stalls.
cat >/dev/null 2>&1 || true

HOOK_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
# Fail soft on a corrupted install: a missing lib must never break the session.
[ -f "$HOOK_DIR/lib/common.sh" ] || exit 0
# shellcheck source=lib/common.sh
. "$HOOK_DIR/lib/common.sh"

plugin_dir="$(cd "$HOOK_DIR/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/skills/jev/scripts/jev.sh}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

reg_root="$(jev_config_root)/jev"
mkdir -p "$reg_root" 2>/dev/null || { echo "jev: cannot create $reg_root — wrapper not published" >&2; exit 0; }

dst="$reg_root/jev.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || { echo "jev: cannot publish $dst" >&2; exit 0; }
fi

# This plugin owns its mandate so first-session delivery does not depend on
# the core plugin, hook ordering, or the model choosing to load a skill. The
# UserPromptSubmit hook repeats a shorter form on every task so the rule
# survives long sessions; this is the full statement.
missing="$(jev_missing_prereqs "$dst")"
if [ -n "$missing" ]; then
  context="Jev unavailable (missing:$missing). Set TYPESAFE_API_KEY in the agent's launch environment and install curl/jq. Jev is mandatory on every task once available; until then, state the limitation once per task and use an explicit reasoning/evidence fallback; never invent a Jev result. Do not read credentials from .env."
else
  context="Jev is mandatory on every task containing semantic decisions. After initial exploration, identify useful judgments over supplied evidence; you MUST call \"$dst\" before the decision it informs. Reassess after new evidence, failed hypotheses, or changed requirements. Batch independent questions in one ask call. Reuse unchanged evidence and questions rather than repeating calls. If a task has no semantic decision, say so in one sentence rather than skipping silently. Syntax and linked examples: $plugin_dir/skills/jev/SKILL.md. Keep exact rules, tests, and code verification deterministic. On service failure, state the limitation and use an explicit evidence fallback. Jev never replaces tests or authorization."
fi
jev_emit SessionStart "$context"

exit 0
