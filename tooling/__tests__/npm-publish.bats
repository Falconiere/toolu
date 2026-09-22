#!/usr/bin/env bats
# npm publish wiring: the CLI is versioned by release-please and published by
# a workflow that reacts to the GitHub Release.

setup() {
  ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  WF="$ROOT/.github/workflows/npm-publish.yml"
}

@test "release-please bumps the CLI package alongside every other package" {
  jq -e '
    [.packages["."]["extra-files"][].path] as $paths
    | ($paths | index("tools/toolu-cli/package.json"))
      and ($paths | index("packages/toolu-core/package.json"))
      and ($paths | index("tools/toolu-opencode/package.json"))
  ' "$ROOT/release-please-config.json"
}

@test "the CLI version matches every other workspace package" {
  root=$(jq -r .version "$ROOT/package.json")
  for p in tools/toolu-cli packages/toolu-core tools/toolu-opencode tools/toolu-conformance; do
    have=$(jq -r .version "$ROOT/$p/package.json")
    [ "$have" = "$root" ] || { echo "$p is $have, root is $root"; return 1; }
  done
}

@test "the publish workflow triggers on a published release" {
  grep -Fq 'types: [published]' "$WF"
}

@test "the publish workflow grants id-token for provenance and publishes with it" {
  grep -Fq 'id-token: write' "$WF"
  grep -Fq 'npm publish --provenance --access public' "$WF"
}

@test "the publish workflow reads the token from secrets, never a literal" {
  grep -Fq 'NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}' "$WF"
  run grep -Eq 'npm_[A-Za-z0-9]{20,}' "$WF"
  [ "$status" -ne 0 ]
}

@test "the publish workflow refuses a tag that disagrees with the package version" {
  grep -Fq 'does not match tools/toolu-cli version' "$WF"
}

@test "only the CLI is published; the library packages stay private" {
  grep -Fq 'working-directory: tools/toolu-cli' "$WF"
  for p in packages/toolu-core tools/toolu-opencode tools/toolu-conformance; do
    jq -e '.private == true' "$ROOT/$p/package.json" >/dev/null
  done
  jq -e '.private == null' "$ROOT/tools/toolu-cli/package.json" >/dev/null
}

@test "the published tarball carries the bundle and the manifest and nothing else" {
  run bun run "$ROOT/tooling/src/pack-inventory.ts"
  [ "$status" -eq 0 ]
}
