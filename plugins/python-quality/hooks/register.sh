#!/usr/bin/env bash
# SessionStart registry sync for the python-quality plugin.
#
# Assembles hooks/concerns/[0-9][0-9]-*.sh (ordered preamble → concern partials
# → finalize) into ONE runtime module in the toolu registry
# (${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/post-tools.d/) under the
# namespaced filename python-quality@toolu__python-quality.sh. The fragments are
# partials of a single script; concatenating them in numeric order yields the
# one module that runs as a single process doing one gate write (authored
# fragment-first — unlike rust/ts-quality there was never a pre-split monolith). Prunes any stale entry bearing OUR prefix, and
# clears our own crashed tmp residue. Other plugins' entries are never touched.
#
# Silent on success (SessionStart stdout becomes context); errors are
# non-fatal — a failed sync means the registry copy is stale, not broken.

SPEC="python-quality@toolu"
OUT="python-quality.sh"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="$SELF_DIR/concerns"
if [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
  CONFIG_ROOT="$TOOLU_CONFIG_DIR"
elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
  CONFIG_ROOT="${CODEX_HOME:-$HOME/.codex}"
else
  CONFIG_ROOT="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
fi
REG_DIR="$CONFIG_ROOT/toolu/post-tools.d"

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

[ -d "$SRC_DIR" ] || exit 0
mkdir -p "$REG_DIR" 2>/dev/null || exit 0

# Clear OUR orphaned atomic-write residue from prior crashed runs. Age-gated
# so a concurrent SessionStart's in-flight tmp (seconds old) is never clobbered.
find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh.tmp.*" -mmin +1 -delete 2>/dev/null

# Assemble: concatenate the numeric-prefixed fragments in lexical order into one
# module. A newline guard between fragments keeps a fragment that lacks a
# trailing newline from running into the next. __tests__/ and any non-prefixed
# .sh are skipped by the glob.
dst="$REG_DIR/${SPEC}__${OUT}"
tmp="${dst}.tmp.$$"
: > "$tmp"
for src in "$SRC_DIR"/[0-9][0-9]-*.sh; do
  [ -f "$src" ] || continue
  cat "$src" >> "$tmp" 2>/dev/null || { rm -f "$tmp"; exit 0; }
  printf '\n' >> "$tmp" 2>/dev/null || { rm -f "$tmp"; exit 0; }
done
if [ -s "$tmp" ] && { [ ! -f "$dst" ] || ! cmp -s "$tmp" "$dst"; }; then
  mv "$tmp" "$dst" 2>/dev/null || rm -f "$tmp"
else
  rm -f "$tmp"
fi

# Prune: remove any entry bearing OUR prefix that is not the current assembled
# module (e.g. per-concern files left by an older version). Never glob outside
# our own spec prefix.
for old in "$REG_DIR/${SPEC}__"*.sh; do
  [ -f "$old" ] || continue
  [ "$(basename "$old")" = "${SPEC}__${OUT}" ] || rm -f "$old"
done

exit 0
