#!/usr/bin/env bash
# SessionStart registry sync for the rust-quality plugin.
#
# Assembles hooks/concerns/[0-9][0-9]-*.sh (ordered preamble → concern partials
# → finalize) into ONE runtime module in the toolu registry
# (${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/post-tools.d/) under the
# namespaced filename rust-quality@toolu__rust-quality.sh. The fragments are
# partials of a single script; concatenating them in numeric order rebuilds the
# original monolith so one process does one gate write (preserving
# byte-identical behavior). Prunes any stale entry bearing OUR prefix, and
# clears our own crashed tmp residue. Other plugins' entries are never touched.
#
# Silent on success (SessionStart stdout becomes context); errors are
# non-fatal — a failed sync means the registry copy is stale, not broken.

SPEC="rust-quality@toolu"
OUT="rust-quality.sh"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_DIR="$SELF_DIR/concerns"
REG_DIR="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/post-tools.d"

# Consume stdin so Claude Code's hook IPC never stalls.
cat > /dev/null 2>&1 || true

[ -d "$SRC_DIR" ] || exit 0
mkdir -p "$REG_DIR" 2>/dev/null || exit 0

# Clear OUR orphaned atomic-write residue from prior crashed runs. Age-gated
# so a concurrent SessionStart's in-flight tmp (seconds old) is never clobbered.
find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh.tmp.*" -mmin +1 -delete 2>/dev/null

# Assemble: concatenate, in order, (1) the preamble fragment 00-*.sh, (2) any
# shared lib/*.sh in lexical order, (3) the remaining numeric-prefixed fragments
# in lexical order — into one module. The lib slot is what makes "define once"
# real: a relocated module cannot source a plugin-relative path, so the shared
# rule library is concatenated INTO the module here while the standalone scanner
# sources the same bytes directly (same source, two consumers, zero drift). A
# plugin with no lib/ dir simply contributes nothing there, leaving the classic
# preamble→concerns order. A newline guard between fragments keeps a fragment
# that lacks a trailing newline from running into the next. __tests__/ and any
# non-prefixed .sh are skipped by the globs.
dst="$REG_DIR/${SPEC}__${OUT}"
tmp="${dst}.tmp.$$"
: > "$tmp"
for src in "$SRC_DIR"/00-*.sh "$SELF_DIR"/lib/*.sh "$SRC_DIR"/[0-9][0-9]-*.sh; do
  [ -f "$src" ] || continue
  case "$src" in "$SRC_DIR"/00-*.sh) [ -z "$_seen_preamble" ] || continue; _seen_preamble=1 ;; esac
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
