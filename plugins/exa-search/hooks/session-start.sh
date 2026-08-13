#!/usr/bin/env bash
# SessionStart hook — publish the exa-search wrapper at a STABLE,
# env-independent path. See context7's session-start.sh for the same fix
# rationale: ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only —
# NOT to the agent's Bash tool subshell — so a SKILL.md path of
# "${CLAUDE_PLUGIN_ROOT}/skills/exa-search/scripts/search.sh" expands to
# "/skills/.../search.sh: No such file" when an agent pastes it. Mirror the
# statusline plugin: symlink the wrapper to
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/exa-search/search.sh
# Refreshed every session; silent on success; every step non-fatal.

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/skills/exa-search/scripts/search.sh}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  config_root="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  config_root="${CODEX_HOME:-$HOME/.codex}"
else
  config_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
reg_root="$config_root/exa-search"
mkdir -p "$reg_root" 2>/dev/null || { echo "exa-search: cannot create $reg_root — wrapper not published" >&2; exit 0; }

dst="$reg_root/search.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || true
fi

exit 0
