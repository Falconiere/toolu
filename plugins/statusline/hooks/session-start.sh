#!/usr/bin/env bash
# SessionStart hook for the statusline plugin.
#
# Claude Code does not let a plugin declare `statusLine` in its manifest, so we
# symlink the script to a stable, version-independent path that settings.json
# can point at without hardcoding the version-specific plugin cache dir:
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/statusline/statusline.sh
# The symlink is refreshed every session, so plugin updates are picked up with
# no settings change. statusline is self-contained — this hook sources no
# toolu libs; the registry root is just the config dir + /statusline.
#
# Silent on success; every step is non-fatal (a failed symlink means the
# statusline is stale, not that the session breaks).

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Resolve the plugin dir from this hook's location: hooks/.. = plugin root.
# If the cd fails, skip rather than symlinking a bogus "/statusline.sh" path.
plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
statusline_src="${plugin_dir:+$plugin_dir/statusline.sh}"
[ -n "$statusline_src" ] && [ -f "$statusline_src" ] || exit 0

if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  config_root="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  config_root="${CODEX_HOME:-$HOME/.codex}"
else
  config_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
reg_root="$config_root/statusline"
# Non-fatal hook: a failure here (e.g. a non-directory file occupies $reg_root)
# must not break the session, but leave one stderr breadcrumb so the otherwise-
# silent no-op is debuggable.
mkdir -p "$reg_root" 2>/dev/null || { echo "statusline: cannot create $reg_root — statusline not wired" >&2; exit 0; }

# Own the path only when it is already our symlink or absent — never clobber a
# real file a user may have placed at $reg_root/statusline.sh. (-L catches a
# broken/relinked symlink that -e would report as missing.)
dst="$reg_root/statusline.sh"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$statusline_src" "$dst" 2>/dev/null || true
fi

exit 0
