#!/usr/bin/env bash
# SessionStart hook — publish the agent-browser wrapper at a STABLE,
# env-independent path.
#
# ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only — NOT to the
# Bash tool's subshell — so SKILL.md's
#   "${CLAUDE_PLUGIN_ROOT}/skills/agent-browser/scripts/agent-browser.sh …"
# expands to "/skills/.../agent-browser.sh: No such file" when an agent pastes
# it. Mirror the statusline plugin: symlink the wrapper to
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/agent-browser/agent-browser.sh
# which the Bash subshell CAN expand. Refreshed every session so plugin
# updates land with no settings change. Silent on success; every step is
# non-fatal (a failed symlink means the published copy is stale, not that
# the session breaks).

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Resolve the plugin dir from this hook's location: hooks/.. = plugin root.
plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/skills/agent-browser/scripts/agent-browser.sh}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

reg_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/agent-browser"
mkdir -p "$reg_root" 2>/dev/null || { echo "agent-browser: cannot create $reg_root — wrapper not published" >&2; exit 0; }

# Own the path only when it is already our symlink or absent — never clobber
# a real file a user may have placed at $reg_root/agent-browser.sh. (-L catches
# a broken/relinked symlink that -e would report as missing.)
dst="$reg_root/agent-browser.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || true
fi

exit 0
