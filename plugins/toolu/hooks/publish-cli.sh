#!/usr/bin/env bash
# SessionStart hook: publish the `toolu-claude` terminal launcher.
#
# Symlinks bin/toolu-claude to a stable, version-independent path on disk:
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu/bin/toolu-claude
# so a user who has that dir on $PATH can run `toolu-claude dashboard --open`
# from anywhere. The symlink is refreshed every session, so plugin updates are
# picked up with no PATH change. Add the dir to PATH once (fish:
# `fish_add_path ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu/bin`).
#
# Silent on success; every step is non-fatal (a failed symlink means the
# launcher is stale, not that the session breaks).

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Resolve the plugin dir from this hook's location: hooks/.. = plugin root.
# If the cd fails, skip rather than symlinking a bogus path.
plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
launcher_src="${plugin_dir:+$plugin_dir/bin/toolu-claude}"
[ -n "$launcher_src" ] && [ -f "$launcher_src" ] || exit 0

bin_dir="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu/bin"
# Non-fatal: a failure here (e.g. a non-directory file occupies the path) must
# not break the session, but leave one stderr breadcrumb so the otherwise-silent
# no-op is debuggable.
mkdir -p "$bin_dir" 2>/dev/null || { echo "toolu: cannot create $bin_dir — toolu-claude not wired" >&2; exit 0; }

# Own the path only when it is already our symlink or absent — never clobber a
# real file the user may have placed there. (-L catches a broken/relinked
# symlink that -e would report as missing.)
dst="$bin_dir/toolu-claude"
if [ -L "$dst" ] || [ ! -e "$dst" ]; then
  ln -sf "$launcher_src" "$dst" 2>/dev/null || true
fi

exit 0
