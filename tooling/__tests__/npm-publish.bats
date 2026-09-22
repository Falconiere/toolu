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

# Provenance attestation needs npm >= 11.5.1 and Node >= 22.14. setup-node
# installs the npm bundled with Node, which is older, so the upgrade is
# load-bearing -- Falconiere/toolu-conventions hit this publishing @toolu/create.
@test "the publish workflow pins Node and upgrades npm high enough for provenance" {
  grep -Fq 'node-version: "22.14"' "$WF"
  grep -Fq 'npm install --global npm@11.5.1' "$WF"
}

@test "the publish workflow reads the token from secrets, never a literal" {
  grep -Fq 'NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}' "$WF"
  run grep -Eq 'npm_[A-Za-z0-9]{20,}' "$WF"
  [ "$status" -ne 0 ]
}

@test "the publish workflow refuses a tag that disagrees with any package version" {
  grep -Fq 'does not match $dir version' "$WF"
}

@test "the three published packages are publishable and conformance stays private" {
  for p in packages/toolu-core tools/toolu-opencode tools/toolu-cli; do
    jq -e '.private == null' "$ROOT/$p/package.json" >/dev/null
    jq -e '.license == "MIT"' "$ROOT/$p/package.json" >/dev/null
    jq -e '.publishConfig.access == "public" and .publishConfig.provenance == true' \
      "$ROOT/$p/package.json" >/dev/null
  done
  # The conformance harness is internal and must never ship.
  jq -e '.private == true' "$ROOT/tools/toolu-conformance/package.json" >/dev/null
}

# @toolu/opencode depends on @toolu/core, and Bun rewrites workspace:* to a
# concrete version at pack time, so core must reach the registry first.
@test "the workflow publishes in dependency order, core before opencode" {
  run bash -c "grep -oE 'packages/toolu-core tools/toolu-opencode tools/toolu-cli' '$WF' | head -1"
  [ "$output" = "packages/toolu-core tools/toolu-opencode tools/toolu-cli" ]
}

@test "a package already on the registry is skipped so a re-run resumes" {
  grep -Fq 'is already published' "$WF"
  grep -Fq 'skipping' "$WF"
}

@test "an npm view failure that is not a 404 fails the job instead of publishing" {
  grep -Fq "grep -q 'E404'" "$WF"
  grep -Fq 'for a reason other than the version being absent' "$WF"
}

@test "the published tarball carries the bundle and the manifest and nothing else" {
  run bun run "$ROOT/tooling/src/pack-inventory.ts"
  [ "$status" -eq 0 ]
}
