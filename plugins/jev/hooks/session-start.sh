#!/usr/bin/env bash
# SessionStart hook — publish the jev wrapper at a STABLE, env-independent
# path. See exa-search's session-start.sh for the same fix rationale:
# ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only — NOT to the
# agent's Bash tool subshell — so a SKILL.md path of
# "${CLAUDE_PLUGIN_ROOT}/skills/jev/scripts/jev.sh" expands to
# "/skills/.../jev.sh: No such file" when an agent pastes it. Symlink the
# wrapper to ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/jev/jev.sh instead.
# Refreshed every session; inject a bounded workflow without network calls.
set -euo pipefail

# Consume stdin so Claude Code's hook IPC never stalls.
cat >/dev/null 2>&1 || true

plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/skills/jev/scripts/jev.sh}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  config_root="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  config_root="${CODEX_HOME:-$HOME/.codex}"
else
  config_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
reg_root="$config_root/jev"
mkdir -p "$reg_root" 2>/dev/null || { echo "jev: cannot create $reg_root — wrapper not published" >&2; exit 0; }

dst="$reg_root/jev.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || { echo "jev: cannot publish $dst" >&2; exit 0; }
fi

# This plugin owns its mandate so first-session delivery does not depend on
# the core plugin, hook ordering, or the model choosing to load a skill.
missing=""
for tool in jq curl; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
[ -n "${TYPESAFE_API_KEY:-}" ] || missing="$missing TYPESAFE_API_KEY"
[ -x "$dst" ] || missing="$missing executable-wrapper"
if [ -n "$missing" ]; then
  context="Jev unavailable (missing:$missing). Set TYPESAFE_API_KEY in the agent's launch environment and install curl/jq. For suitable semantic decisions, state the limitation and use an explicit fallback; never invent a Jev result. Do not read credentials from .env."
else
  context="Jev workflow: during brainstorm, spec, spec review, plan, plan review, execution, and review, identify useful bounded semantic decisions (classify, rank, route, or judge supplied evidence). You MUST call \"$dst\" only when the answer would change the next action; read $plugin_dir/skills/jev/SKILL.md for syntax. Batch independent questions over shared state in one ask call. Keep exact rules, arithmetic, tests, and code verification deterministic. Use minimal relevant state; reuse answers only while evidence and questions are unchanged. On service failure or uncertainty, state the limitation and use an explicit fallback. Jev is evidence, never a replacement for tests or authorization."
fi
if command -v jq >/dev/null 2>&1; then
  jq -nc --arg context "$context" '{hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: $context}}'
else
  # Both hosts also accept plain stdout as SessionStart context.
  printf '%s\n' "$context"
fi

exit 0
