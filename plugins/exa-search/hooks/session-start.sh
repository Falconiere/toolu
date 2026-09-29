#!/usr/bin/env bash
# SessionStart hook — publish the exa-search wrapper at a STABLE,
# env-independent path. See context7's session-start.sh for the same fix
# rationale: ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses only —
# NOT to the agent's Bash tool subshell — so any SKILL.md path built on it
# (e.g. "${CLAUDE_PLUGIN_ROOT}/hooks/dist/search.js") would expand to
# "/hooks/dist/search.js: No such file" when an agent pastes it. Mirror the
# statusline plugin: symlink the CLI bundle (hooks/dist/search.js) to
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/exa-search/search.sh
# Refreshed every session; silent on success; every step non-fatal.

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

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
reg_root="$config_root/exa-search"
mkdir -p "$reg_root" 2>/dev/null || { echo "exa-search: cannot create $reg_root — wrapper not published" >&2; exit 0; }

dst="$reg_root/search.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$src" "$dst" 2>/dev/null || true
fi

# The published path is a Bun bundle run through its `#!/usr/bin/env bun`
# shebang: without bun on PATH every call fails with a bare "env: bun: No
# such file", so say what is missing now (advisory; the session continues).
command -v bun >/dev/null 2>&1 || echo "exa-search: bun not found on PATH — the exa-search search CLI needs Bun 1.4.x (https://bun.sh; see docs/runtime.md)" >&2

exit 0
