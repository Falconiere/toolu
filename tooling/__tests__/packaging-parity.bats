#!/usr/bin/env bats
# Validates real plugin manifests and host marketplace catalogs together.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SCRIPT="$ROOT/tooling/validate-plugin-packaging.sh"

@test "plugin packaging validator accepts the checked-in dual-host catalog" {
  run bash "$SCRIPT"
  [ "$status" -eq 0 ]
  [[ "$output" == *"validated 14 plugins"* ]]
}

@test "plugin packaging validator rejects a release config that omits a Codex manifest" {
  repo=$(mktemp -d)
  cp "$ROOT/package.json" "$ROOT/release-please-config.json" "$repo/"
  cp -R "$ROOT/plugins" "$ROOT/.claude-plugin" "$ROOT/.agents" "$repo/"
  jq 'del(.packages["."]."extra-files"[] | select(.path == "plugins/toolu/.codex-plugin/plugin.json"))' \
    "$repo/release-please-config.json" > "$repo/release-please-config.json.tmp"
  mv "$repo/release-please-config.json.tmp" "$repo/release-please-config.json"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"release-please is missing"* ]]
}

@test "plugin packaging validator rejects marketplace descriptions that drift from manifests" {
  repo=$(mktemp -d)
  cp "$ROOT/package.json" "$ROOT/release-please-config.json" "$repo/"
  cp -R "$ROOT/plugins" "$ROOT/.claude-plugin" "$ROOT/.agents" "$repo/"
  jq '(.plugins[] | select(.name == "toolu") | .description) = "stale description"' \
    "$repo/.claude-plugin/marketplace.json" > "$repo/.claude-plugin/marketplace.json.tmp"
  mv "$repo/.claude-plugin/marketplace.json.tmp" "$repo/.claude-plugin/marketplace.json"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"marketplace description differs"* ]]
}

@test "plugin packaging validator rejects malformed Codex agent TOML" {
  repo=$(mktemp -d)
  cp "$ROOT/package.json" "$ROOT/release-please-config.json" "$repo/"
  cp -R "$ROOT/plugins" "$ROOT/.claude-plugin" "$ROOT/.agents" "$repo/"
  printf '%s\n' 'model = [' >> "$repo/plugins/toolu/assets/agents/architect.toml"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid agent TOML"* ]]
}

@test "plugin packaging validator rejects symlinks that escape a plugin root" {
  repo=$(mktemp -d)
  cp "$ROOT/package.json" "$ROOT/release-please-config.json" "$repo/"
  cp -R "$ROOT/plugins" "$ROOT/.claude-plugin" "$ROOT/.agents" "$repo/"
  ln -s /etc/hosts "$repo/plugins/toolu/escaping-link"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"symlink escapes plugin root"* ]]
}
