#!/usr/bin/env bats
# The install-everything prompts in the README and plugin index stay identical
# and name every plugin in the Claude marketplace catalog.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"

extract_region() {
  local file="$1" host="$2"
  awk -v start="<!-- install-everything:${host} -->" \
      -v end="<!-- /install-everything:${host} -->" '
    $0 == start { capture = 1; next }
    $0 == end { capture = 0; next }
    capture { print }
  ' "$file"
}

@test "install-everything fences match in the README and the plugin index" {
  local host readme docs
  for host in claude codex opencode; do
    readme="$(extract_region "$ROOT/README.md" "$host")"
    docs="$(extract_region "$ROOT/docs/plugins/index.md" "$host")"
    [ -n "$readme" ]
    [ "$readme" = "$docs" ]
  done
}

@test "install-everything fences list the marketplace plugins, core first" {
  local host fence tokens catalog first
  catalog="$(jq -r '.plugins[].name' "$ROOT/.claude-plugin/marketplace.json" | sort)"
  [ -n "$catalog" ]
  for host in claude codex; do
    fence="$(extract_region "$ROOT/README.md" "$host")"
    [ -n "$fence" ]
    ! printf '%s\n' "$fence" | grep -q 'comemory'
    tokens="$(printf '%s\n' "$fence" | grep -oE '[a-z0-9-]+@toolu' | sed 's/@toolu$//' | sort)"
    [ "$tokens" = "$catalog" ]
    first="$(printf '%s\n' "$fence" | grep -oE '[a-z0-9-]+@toolu' | head -n 1)"
    [ "$first" = "toolu@toolu" ]
  done
}

@test "install-everything opencode fence is git-clone wiring, not marketplace plugins" {
  local fence
  fence="$(extract_region "$ROOT/README.md" "opencode")"
  [ -n "$fence" ]
  ! printf '%s\n' "$fence" | grep -q 'comemory'
  ! printf '%s\n' "$fence" | grep -qE '[a-z0-9-]+@toolu'
  printf '%s\n' "$fence" | grep -q 'TOOLU_REPO_ROOT'
  printf '%s\n' "$fence" | grep -q '@toolu/opencode'
  printf '%s\n' "$fence" | grep -q 'bun install --frozen-lockfile'
  printf '%s\n' "$fence" | grep -q 'tools/toolu-opencode/generated/skills'
}
