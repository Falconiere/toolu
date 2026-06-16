#!/usr/bin/env bash
# SessionStart registry sync for the ast-grep plugin.
#
# Mirrors hooks/pre-tools.d/*.sh and hooks/post-tools.d/*.sh into the toolu
# runtime registry (${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/{pre,post}-tools.d/)
# under the namespaced filename ast-grep@toolu__<name>.sh, and prunes entries
# bearing OUR prefix whose source module no longer exists. Other plugins'
# entries are never touched. The core toolu dispatcher executes the synced
# copies, gated on this plugin being installed.
#
# Silent on success (SessionStart stdout becomes context); errors are
# non-fatal — a failed sync means the registry copy is stale, not broken.

SPEC="ast-grep@toolu"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
REG_ROOT="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu"

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Mirror one source dir into its matching registry dir (atomic, self-pruning).
sync_dir() {
  local src_dir="$1" reg_dir="$2" src name dst tmp
  [ -d "$src_dir" ] || return 0
  mkdir -p "$reg_dir" 2>/dev/null || return 0

  # Clear OUR orphaned atomic-write residue from prior crashed runs (a death
  # between cp and mv leaves <spec>__<name>.sh.tmp.<pid>; nothing executes
  # them, but nothing else cleans them either). Age-gated so a concurrent
  # SessionStart's in-flight tmp (seconds old) is never clobbered.
  find "$reg_dir" -maxdepth 1 -name "${SPEC}__*.sh.tmp.*" -mmin +1 -delete 2>/dev/null

  # Sync: copy each source module if missing or changed (atomic tmp+mv).
  for src in "$src_dir"/*.sh; do
    [ -f "$src" ] || continue
    name=$(basename "$src")
    dst="$reg_dir/${SPEC}__${name}"
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
  for dst in "$reg_dir/${SPEC}__"*.sh; do
    [ -f "$dst" ] || continue
    name=$(basename "$dst")
    src="$src_dir/${name#"${SPEC}"__}"
    [ -f "$src" ] || rm -f "$dst"
  done
}

sync_dir "$SELF_DIR/pre-tools.d"  "$REG_ROOT/pre-tools.d"
sync_dir "$SELF_DIR/post-tools.d" "$REG_ROOT/post-tools.d"

exit 0
