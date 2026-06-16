#!/usr/bin/env bash
# SessionStart hook — publish the code-review write-state helper at a STABLE,
# env-independent path. See context7's session-start.sh for the same fix
# rationale: ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only —
# NOT to the agent's Bash tool subshell — so a SKILL.md path of
# "${CLAUDE_PLUGIN_ROOT}/skills/review/scripts/write-state.sh" expands to
# "/skills/.../write-state.sh: No such file" when an agent pastes it.
# Mirror the statusline plugin: symlink the helper to
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/code-review/write-state.sh
# Refreshed every session; silent on success; every step non-fatal.

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/skills/review/scripts/write-state.sh}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

reg_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/code-review"
mkdir -p "$reg_root" 2>/dev/null || { echo "code-review: cannot create $reg_root — helper not published" >&2; exit 0; }

dst="$reg_root/write-state.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || true
fi

exit 0
