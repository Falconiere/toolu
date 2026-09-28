#!/usr/bin/env bash
# SessionStart hook — publish the context7 search wrapper at a STABLE,
# env-independent path.
#
# ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only — NOT to the
# Bash tool's subshell — so any SKILL.md path built on it, e.g.
#   "${CLAUDE_PLUGIN_ROOT}/hooks/dist/search.js …"
# would expand to "/hooks/dist/search.js: No such file" when an agent pastes it.
# Mirror the statusline plugin: symlink the CLI bundle (hooks/dist/search.js) to
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/context7/search.sh
# which the Bash subshell CAN expand. Refreshed every session so plugin
# updates land with no settings change. Silent on success; every step is
# non-fatal (a failed symlink means the published copy is stale, not that
# the session breaks).

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Resolve the plugin dir from this hook's location: hooks/.. = plugin root.
plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
src="${plugin_dir:+$plugin_dir/hooks/dist/search.js}"
[ -n "$src" ] && [ -f "$src" ] || exit 0

if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  config_root="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  config_root="${CODEX_HOME:-$HOME/.codex}"
else
  config_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
reg_root="$config_root/context7"
mkdir -p "$reg_root" 2>/dev/null || { echo "context7: cannot create $reg_root — wrapper not published" >&2; exit 0; }

# Own the path only when it is already our symlink or absent — never clobber
# a real file a user may have placed at $reg_root/search.sh. (-L catches a
# broken/relinked symlink that -e would report as missing.)
dst="$reg_root/search.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || true
fi

# The published path is a Bun bundle run through its `#!/usr/bin/env bun`
# shebang: without bun on PATH every call fails with a bare "env: bun: No
# such file", so say what is missing now (advisory; the session continues).
command -v bun >/dev/null 2>&1 || echo "context7: bun not found on PATH — the context7 search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)" >&2

exit 0
