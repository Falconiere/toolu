#!/usr/bin/env bash
# SessionStart hook — publish the design detector + references at a STABLE,
# env-independent path. ${CLAUDE_PLUGIN_ROOT} is exported to hook subprocesses
# only — NOT to the agent's Bash tool subshell — so skills must read from
#   ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design/{detect-stack.sh,references/}
# Refreshed every session; silent on success; every step non-fatal.

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

plugin_dir="$(cd "$(dirname "$0")/.." 2>/dev/null && pwd)"
[ -n "$plugin_dir" ] || exit 0
src_script="$plugin_dir/scripts/detect-stack.sh"
src_refs="$plugin_dir/references"
[ -f "$src_script" ] || exit 0

reg_root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/design"
mkdir -p "$reg_root" 2>/dev/null || { echo "design: cannot create $reg_root — not published" >&2; exit 0; }

# File symlink: plain `ln -sf` is correct for a file target.
dst_script="$reg_root/detect-stack.sh"
if [ -L "$dst_script" ] || [ ! -e "$dst_script" ]; then
  ln -sf "$src_script" "$dst_script" 2>/dev/null || true
fi

# Directory symlink: MUST use `ln -sfn` (no-dereference). Plain `ln -sf SRC DST`
# when DST is an existing symlink-to-dir dereferences it and creates a nested
# DST/references — `-n` replaces the link instead. Only touch our own symlink
# or an absent path; never clobber a real directory.
dst_refs="$reg_root/references"
if [ -d "$src_refs" ] && { [ -L "$dst_refs" ] || [ ! -e "$dst_refs" ]; }; then
  ln -sfn "$src_refs" "$dst_refs" 2>/dev/null || true
fi

exit 0
