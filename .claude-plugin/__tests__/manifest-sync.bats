#!/usr/bin/env bats
# Drift guard: toolu's plugin dependencies are declared in TWO manifests that
# must stay in lockstep -- the marketplace toolu entry's `dependencies` array
# (.claude-plugin/marketplace.json) and the plugin's own top-level
# `dependencies` (plugins/toolu/.claude-plugin/plugin.json). There is no stable
# build step syncing them, so this test fails if they drift. Both are also
# asserted empty: toolu is the workflow core and depends on nothing.

ROOT="${BATS_TEST_DIRNAME}/../.."   # .claude-plugin/__tests__ -> .claude-plugin -> repo root

@test "toolu dependencies are in sync across marketplace.json and plugin.json, and both empty" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  marketplace="$ROOT/.claude-plugin/marketplace.json"
  plugin="$ROOT/plugins/toolu/.claude-plugin/plugin.json"
  [ -f "$marketplace" ]
  [ -f "$plugin" ]

  m=$(jq -cS '.plugins[]|select(.name=="toolu").dependencies' "$marketplace")
  p=$(jq -cS '.dependencies' "$plugin")
  [ -n "$m" ]
  [ -n "$p" ]

  # Drift guard: the two arrays must be equal.
  [ "$m" = "$p" ]

  # Both must be empty.
  [ "$m" = '[]' ]
  [ "$p" = '[]' ]
}
