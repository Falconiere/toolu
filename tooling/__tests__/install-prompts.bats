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

# The fences used to enumerate every plugin as `<name>@toolu`, core first, and
# this test compared that list against the catalog. The CLI now derives both the
# set and the order from .claude-plugin/marketplace.json itself -- covered by
# tools/toolu-cli/src/catalog/__tests__/order.test.ts -- so the README must NOT
# hand-enumerate them: a second copy of the catalog is exactly the drift the
# version column already taught us about.
@test "install-everything fences drive the CLI instead of enumerating plugins" {
  local host fence
  for host in claude codex; do
    fence="$(extract_region "$ROOT/README.md" "$host")"
    [ -n "$fence" ]
    printf '%s\n' "$fence" | grep -q 'npx @toolu/cli plugins install'
    ! printf '%s\n' "$fence" | grep -q 'comemory'
    ! printf '%s\n' "$fence" | grep -qE '[a-z0-9-]+@toolu'
  done
}

@test "the codex fence targets codex and the claude fence does not" {
  printf '%s\n' "$(extract_region "$ROOT/README.md" codex)" | grep -q -- '--host codex'
  ! printf '%s\n' "$(extract_region "$ROOT/README.md" claude)" | grep -q -- '--host'
}

@test "install-everything opencode fence is git-clone wiring, not marketplace plugins" {
  local fence
  fence="$(extract_region "$ROOT/README.md" "opencode")"
  [ -n "$fence" ]
  # Mentions the ban explicitly (same intent as Claude/Codex omitting comemory installs).
  printf '%s\n' "$fence" | grep -q 'Do not install comemory via toolu'
  ! printf '%s\n' "$fence" | grep -qE '[a-z0-9-]+@toolu'
  printf '%s\n' "$fence" | grep -q 'TOOLU_REPO_ROOT'
  printf '%s\n' "$fence" | grep -q '@toolu/opencode'
  printf '%s\n' "$fence" | grep -q 'bun install --frozen-lockfile'
  printf '%s\n' "$fence" | grep -q 'tools/toolu-opencode/generated/skills'
}

# The README used to carry a per-plugin version column that release-please never
# updated, so it silently went stale at every release. The column is gone; this
# keeps a hardcoded repository version from creeping back into the file.
@test "README does not hardcode the repository version" {
  root=$(jq -r .version "$ROOT/package.json")
  run grep -Fc "$root" "$ROOT/README.md"
  [ "$output" = "0" ]
}
