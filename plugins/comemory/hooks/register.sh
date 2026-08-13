#!/usr/bin/env bash
# SessionStart registry sync for the comemory plugin.
#
# Mirrors hooks/pre-tools.d/*.sh into the toolu runtime registry
# (${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/pre-tools.d/) under the
# namespaced filename comemory@toolu__<name>.sh, and prunes entries
# bearing OUR prefix whose source module no longer exists. Other plugins'
# entries are never touched. The core toolu dispatcher executes the
# synced copies, gated on this plugin being installed.
#
# Silent on success (SessionStart stdout becomes context); errors are
# non-fatal — a failed sync means the registry copy is stale, not broken.

SPEC="comemory@toolu"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="$SELF_DIR/pre-tools.d"
if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  CONFIG_ROOT="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  CONFIG_ROOT="${CODEX_HOME:-$HOME/.codex}"
else
  CONFIG_ROOT="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
REG_DIR="$CONFIG_ROOT/toolu/pre-tools.d"

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

[ -d "$SRC_DIR" ] || exit 0
mkdir -p "$REG_DIR" 2>/dev/null || exit 0

# Clear OUR orphaned atomic-write residue from prior crashed runs (a death
# between cp and mv leaves <spec>__<name>.sh.tmp.<pid>; nothing executes
# them, but nothing else cleans them either). Age-gated so a concurrent
# SessionStart's in-flight tmp (seconds old) is never clobbered.
find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh.tmp.*" -mmin +1 -delete 2>/dev/null

# Sync: copy each source module if missing or changed (atomic tmp+mv).
for src in "$SRC_DIR"/*.sh; do
  [ -f "$src" ] || continue
  name=$(basename "$src")
  dst="$REG_DIR/${SPEC}__${name}"
  if [ ! -f "$dst" ] || ! cmp -s "$src" "$dst"; then
    tmp="${dst}.tmp.$$"
    if cp "$src" "$tmp" 2>/dev/null; then
      mv "$tmp" "$dst" 2>/dev/null || rm -f "$tmp"
    else
      rm -f "$tmp" 2>/dev/null
    fi
  fi
done

# Prune: remove OUR entries whose source module is gone. Never glob outside
# our own spec prefix.
for dst in "$REG_DIR/${SPEC}__"*.sh; do
  [ -f "$dst" ] || continue
  name=$(basename "$dst")
  src="$SRC_DIR/${name#"${SPEC}"__}"
  [ -f "$src" ] || rm -f "$dst"
done

# Publish the scoped wrapper at a STABLE, env-independent path so the agent's
# Bash tool can invoke it without ${CLAUDE_PLUGIN_ROOT} (that var is set for
# hook subprocesses, NOT for the Bash tool's subshell — an agent that pastes
# `${CLAUDE_PLUGIN_ROOT}/skills/.../comemory.sh` from SKILL.md hits an empty
# expansion, runs `/skills/.../comemory.sh: No such file`, and misreads the
# 0-byte stdout as "no memory hits"). Mirror the statusline plugin's pattern:
#   ${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/comemory/comemory.sh
# Refreshed every SessionStart so plugin updates are picked up.
PLUGIN_ROOT="$(cd "$SELF_DIR/.." 2>/dev/null && pwd)"
wrapper_src="${PLUGIN_ROOT:+$PLUGIN_ROOT/skills/agent-memory/scripts/comemory.sh}"
if [ -n "$wrapper_src" ] && [ -f "$wrapper_src" ]; then
  pub_root="$CONFIG_ROOT/comemory"
  if mkdir -p "$pub_root" 2>/dev/null; then
    pub_dst="$pub_root/comemory.sh"
    # Own the path only when it is already our symlink or absent — never
    # clobber a real file a user may have placed at that path (-L catches a
    # broken/relinked symlink that -e would report as missing).
    if [ -L "$pub_dst" ] || [ ! -e "$pub_dst" ]; then
      ln -sf "$wrapper_src" "$pub_dst" 2>/dev/null || true
    fi
  fi
fi

exit 0
