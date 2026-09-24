#!/usr/bin/env bats
# Validates real plugin manifests and host marketplace catalogs together.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
SCRIPT="$ROOT/tooling/validate-plugin-packaging.sh"

copy_packaging_fixture() {
  local repo=$1
  cp "$ROOT/package.json" "$ROOT/release-please-config.json" "$repo/"
  cp -R "$ROOT/plugins" "$ROOT/.claude-plugin" "$ROOT/.agents" "$repo/"
  mkdir -p "$repo/packages/toolu-core" "$repo/tools/toolu-opencode" "$repo/tools/toolu-conformance"
  cp "$ROOT/packages/toolu-core/package.json" "$repo/packages/toolu-core/"
  cp "$ROOT/tools/toolu-opencode/package.json" "$repo/tools/toolu-opencode/"
  cp "$ROOT/tools/toolu-conformance/package.json" "$repo/tools/toolu-conformance/"
}

@test "plugin packaging validator accepts the checked-in dual-host catalog" {
  run bash "$SCRIPT"
  if [ "$status" -ne 0 ]; then
    printf 'packaging validator failed:\n%s\n' "$output" >&2
  fi
  [ "$status" -eq 0 ]
  [[ "$output" == *"validated 14 plugins"* ]]
}

@test "plugin packaging validator rejects a release config that omits a Codex manifest" {
  repo=$(mktemp -d)
  copy_packaging_fixture "$repo"
  jq 'del(.packages["."]."extra-files"[] | select(.path == "plugins/toolu/.codex-plugin/plugin.json"))' \
    "$repo/release-please-config.json" > "$repo/release-please-config.json.tmp"
  mv "$repo/release-please-config.json.tmp" "$repo/release-please-config.json"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"release-please is missing"* ]]
}

@test "plugin packaging validator rejects marketplace descriptions that drift from manifests" {
  repo=$(mktemp -d)
  copy_packaging_fixture "$repo"
  jq '(.plugins[] | select(.name == "toolu") | .description) = "stale description"' \
    "$repo/.claude-plugin/marketplace.json" > "$repo/.claude-plugin/marketplace.json.tmp"
  mv "$repo/.claude-plugin/marketplace.json.tmp" "$repo/.claude-plugin/marketplace.json"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"marketplace description differs"* ]]
}

@test "plugin packaging validator rejects malformed Codex agent TOML" {
  repo=$(mktemp -d)
  copy_packaging_fixture "$repo"
  printf '%s\n' 'model = [' >> "$repo/plugins/toolu/assets/agents/architect.toml"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"invalid agent TOML"* ]]
}

@test "plugin packaging validator rejects symlinks that escape a plugin root" {
  repo=$(mktemp -d)
  copy_packaging_fixture "$repo"
  ln -s /etc/hosts "$repo/plugins/toolu/escaping-link"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"symlink escapes plugin root"* ]]
}

@test "plugin packaging validator rejects workspace package.json version drift" {
  repo=$(mktemp -d)
  copy_packaging_fixture "$repo"
  jq '.version = "0.0.0"' "$repo/packages/toolu-core/package.json" > "$repo/packages/toolu-core/package.json.tmp"
  mv "$repo/packages/toolu-core/package.json.tmp" "$repo/packages/toolu-core/package.json"

  run env PACKAGING_ROOT="$repo" bash "$SCRIPT"
  [ "$status" -ne 0 ]
  [[ "$output" == *"packages/toolu-core/package.json version differs"* ]]
}
