#!/usr/bin/env bash
# SessionStart registry sync for the git-better plugin.
#
# Mirrors hooks/pre-tools.d/*.sh and hooks/post-tools.d/*.sh into the toolu
# runtime registry (${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/{pre,post}-tools.d/)
# under the namespaced filename git-better@toolu__<name>.sh, prunes entries
# bearing OUR prefix whose source module is gone, and installs/refreshes the
# stable `gb` shim that execs the versioned wrapper. The core toolu dispatcher
# executes the synced copies, gated on this plugin being installed.
#
# Silent on success (SessionStart stdout becomes context); errors are
# non-fatal — a failed sync means the registry copy is stale, not broken.

SPEC="git-better@toolu"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  CONFIG_ROOT="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  CONFIG_ROOT="${CODEX_HOME:-$HOME/.codex}"
  HOST=codex
else
  CONFIG_ROOT="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  HOST=claude
fi
[ -n "${HOST:-}" ] || {
  if [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
    HOST=codex
  else
    HOST=claude
  fi
}
REG_ROOT="$CONFIG_ROOT/toolu"

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

# Mirror one source dir into its matching registry dir (atomic, self-pruning).
sync_dir() {
  local src_dir="$1" reg_dir="$2" src name dst tmp
  [ -d "$src_dir" ] || return 0
  mkdir -p "$reg_dir" 2>/dev/null || return 0

  find "$reg_dir" -maxdepth 1 -name "${SPEC}__*.sh.tmp.*" -mmin +1 -delete 2>/dev/null

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

  for dst in "$reg_dir/${SPEC}__"*.sh; do
    [ -f "$dst" ] || continue
    name=$(basename "$dst")
    src="$src_dir/${name#"${SPEC}"__}"
    [ -f "$src" ] || rm -f "$dst"
  done
}

# Install/refresh the stable `gb` shim → execs the versioned wrapper. The shim
# path is version-stable; this rewrites it whenever the plugin version moves
# (SessionStart re-runs), so `gb` always points at the installed wrapper.
install_shim() {
  local script="$1" host="$2" bin_dir shim want
  bin_dir="$CONFIG_ROOT/toolu/bin"
  mkdir -p "$bin_dir" 2>/dev/null || return 0
  shim="$bin_dir/gb"
  want="#!/usr/bin/env bash
export TOOLU_HOST_OVERRIDE=\"$host\"
exec bash \"$script\" \"\$@\""
  if [ ! -f "$shim" ] || [ "$(cat "$shim" 2>/dev/null)" != "$want" ]; then
    if printf '%s\n' "$want" > "$shim.tmp.$$" 2>/dev/null; then
      chmod +x "$shim.tmp.$$" 2>/dev/null
      mv "$shim.tmp.$$" "$shim" 2>/dev/null || rm -f "$shim.tmp.$$"
    fi
  fi
}

sync_dir "$SELF_DIR/pre-tools.d"  "$REG_ROOT/pre-tools.d"
sync_dir "$SELF_DIR/post-tools.d" "$REG_ROOT/post-tools.d"

gb_script="$(cd "$SELF_DIR/.." && pwd)/scripts/git-better.sh"
[ -f "$gb_script" ] && install_shim "$gb_script" "$HOST"

exit 0
