#!/usr/bin/env bats
# The published @toolu/opencode carries the bash plugins/ tree, because npm
# cannot reach outside a package directory and the bridge enforces those gates.

# Lives in tooling/, not beside the script it tests: Bun symlinks workspace
# packages into each other's node_modules, so a .bats file inside
# tools/toolu-opencode/ is discovered twice -- once for real and once through
# tools/toolu-conformance/node_modules/@toolu/opencode/, where the relative
# walk up to the repo root lands inside node_modules and the script is absent.
setup() {
  ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  SCRIPT="$ROOT/tools/toolu-opencode/scripts/bundle-plugins.sh"
  # Stage into a private directory: pack-inventory triggers the same script
  # through prepack, and the suite runs bats files in parallel.
  DEST="$BATS_TEST_TMPDIR/staged"
  export BUNDLE_PLUGINS_DEST="$DEST"
}

@test "bundles every marketplace plugin" {
  run bash "$SCRIPT"
  [ "$status" -eq 0 ]
  catalog=$(jq -r '.plugins | length' "$ROOT/.claude-plugin/marketplace.json")
  staged=$(find "$DEST" -name plugin.json -path '*.claude-plugin*' | wc -l | tr -d ' ')
  [ "$staged" = "$catalog" ]
}

@test "the staged tree carries the hook engine the bridge actually runs" {
  bash "$SCRIPT" >/dev/null
  [ -f "$DEST/toolu/hooks/hooks.json" ]
  [ -f "$DEST/toolu/hooks/pre-tools/mod.sh" ]
  [ -d "$DEST/toolu/settings" ]
}

@test "colocated tests are excluded from the published copy" {
  bash "$SCRIPT" >/dev/null
  run bash -c "find '$DEST' -type d -name __tests__ | wc -l | tr -d ' '"
  [ "$output" = "0" ]
  # The source tree really does have them, so the exclusion is doing work.
  run bash -c "find '$ROOT/plugins' -type d -name __tests__ | wc -l | tr -d ' '"
  [ "$output" -gt 0 ]
}

@test "a re-run replaces the copy rather than accumulating stale files" {
  bash "$SCRIPT" >/dev/null
  touch "$DEST/stale-marker"
  bash "$SCRIPT" >/dev/null
  [ ! -f "$DEST/stale-marker" ]
}

@test "the package's own staged copy is gitignored so it never lands in a commit" {
  run git -C "$ROOT" check-ignore -q "$ROOT/tools/toolu-opencode/plugins"
  [ "$status" -eq 0 ]
}
