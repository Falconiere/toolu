#!/usr/bin/env bash
# Copies the repository's bash plugins/ tree into this package before packing.
#
# The tree lives at the repository root, and npm cannot include files outside a
# package directory, so a published @toolu/opencode would otherwise have nothing
# to enforce. The copy is gitignored and regenerated on every pack.
set -euo pipefail

PKG_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
REPO_ROOT=$(cd "$PKG_DIR/../.." && pwd)
SRC="$REPO_ROOT/plugins"
# Overridable so a test can stage into its own directory: the suite runs bats
# files in parallel, and two of them staging into the package at once would race.
DEST="${BUNDLE_PLUGINS_DEST:-$PKG_DIR/plugins}"

[ -d "$SRC" ] || { echo "bundle-plugins: $SRC not found" >&2; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST"
# -R follows the tree; __tests__ are excluded because a published plugin never
# runs them and they are a third of the bytes.
(cd "$SRC" && tar --exclude='__tests__' -cf - .) | (cd "$DEST" && tar -xf -)

count=$(find "$DEST" -name plugin.json -path '*.claude-plugin*' | wc -l | tr -d ' ')
[ "$count" -ge 13 ] || { echo "bundle-plugins: only $count plugin manifests copied" >&2; exit 1; }
echo "bundle-plugins: $count plugins staged in $DEST"
