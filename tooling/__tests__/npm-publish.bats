#!/usr/bin/env bats
# npm publish wiring: the CLI is versioned by release-please and published by
# a workflow release-please calls once the Release exists.

setup() {
  ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  WF="$ROOT/.github/workflows/npm-publish.yml"
}

@test "release-please bumps the CLI package alongside every other package" {
  jq -e '
    [.packages["."]["extra-files"][].path] as $paths
    | ($paths | index("tools/toolu-cli/package.json"))
      and ($paths | index("tools/toolu-cli/npm/package.json"))
      and ($paths | index("packages/toolu-core/package.json"))
      and ($paths | index("tools/toolu-opencode/package.json"))
  ' "$ROOT/release-please-config.json"
}

@test "the CLI version matches every other workspace package" {
  root=$(jq -r .version "$ROOT/package.json")
  for p in tools/toolu-cli tools/toolu-cli/npm packages/toolu-core tools/toolu-opencode tools/toolu-conformance; do
    have=$(jq -r .version "$ROOT/$p/package.json")
    [ "$have" = "$root" ] || { echo "$p is $have, root is $root"; return 1; }
  done
}

# release-please creates the Release with the default GITHUB_TOKEN, and GitHub
# does not start a workflow run from an event created by that token. A
# `release: published` trigger here is silently ignored -- that is exactly why
# the 6.7.0 release published nothing.
@test "the publish workflow is chained from release-please, not triggered by the release event" {
  RP="$ROOT/.github/workflows/release-please.yml"
  grep -Fq 'uses: ./.github/workflows/npm-publish.yml' "$RP"
  grep -Fq "needs.release-please.outputs.releases_created == 'true'" "$RP"
  grep -Fq 'workflow_call:' "$WF"
  # The trigger that cannot fire must not come back.
  run grep -Fq 'types: [published]' "$WF"
  [ "$status" -ne 0 ]
}

@test "the publish workflow stays runnable by hand for a given tag" {
  grep -Fq 'workflow_dispatch:' "$WF"
  grep -Fq 'ref: ${{ inputs.tag }}' "$WF"
}

# A tag cut before a package became publishable must still be retryable.
@test "private packages are skipped rather than failing the run" {
  run bash -c "grep -c 'is private at' '$WF'"
  [ "$output" = "2" ]
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
  for p in packages/toolu-core tools/toolu-opencode tools/toolu-cli/npm; do
    jq -e '.private == null' "$ROOT/$p/package.json" >/dev/null
    jq -e '.license == "MIT"' "$ROOT/$p/package.json" >/dev/null
    jq -e '.publishConfig.access == "public" and .publishConfig.provenance == true' \
      "$ROOT/$p/package.json" >/dev/null
  done
  # The conformance harness is internal and must never ship.
  jq -e '.private == true' "$ROOT/tools/toolu-conformance/package.json" >/dev/null
  # The CLI's dev workspace must never ship, and must not carry the published
  # name: a workspace called @toolu/plugins would shadow the registry for npx.
  jq -e '.private == true and .name != "@toolu/plugins"' "$ROOT/tools/toolu-cli/package.json" >/dev/null
}

# @toolu/opencode depends on @toolu/core, and Bun rewrites workspace:* to a
# concrete version at pack time, so core must reach the registry first.
@test "the workflow publishes in dependency order, core before opencode" {
  run bash -c "grep -oE 'packages/toolu-core tools/toolu-opencode tools/toolu-cli/npm' '$WF' | head -1"
  [ "$output" = "packages/toolu-core tools/toolu-opencode tools/toolu-cli/npm" ]
}

@test "a package already on the registry is skipped so a re-run resumes" {
  grep -Fq 'is already published' "$WF"
  grep -Fq 'skipping' "$WF"
}

# @toolu/opencode declares a concrete @toolu/core version. Publishing it after
# core failed would put a package on the registry whose dependency is absent.
@test "a failed publish aborts instead of continuing to dependent packages" {
  grep -Fq 'stopping before its dependents' "$WF"
  # No accumulate-and-continue: the old loop set a status flag and carried on.
  run grep -Fq 'status=1' "$WF"
  [ "$status" -ne 0 ]
}

@test "an npm view failure that is not a 404 fails the job instead of publishing" {
  grep -Fq "grep -q 'E404'" "$WF"
  grep -Fq 'for a reason other than the version being absent' "$WF"
}

@test "the published tarball carries the bundle and the manifest and nothing else" {
  run bun run "$ROOT/tooling/src/pack-inventory.ts"
  [ "$status" -eq 0 ]
}

# npm rejected the unscoped name `toolu` as too similar to the existing package
# `toml` (E403 at publish time, after a 404 had suggested it was free). The
# similarity filter applies to unscoped names only, so the CLI is scoped. It
# declares exactly one bin: npx runs the only bin, which is what makes
# `npx @toolu/plugins install` hand `install` to the CLI.
@test "the CLI publishes as @toolu/plugins with toolu as its only command" {
  jq -e '.name == "@toolu/plugins"' "$ROOT/tools/toolu-cli/npm/package.json" >/dev/null
  jq -e '(.bin | length) == 1 and .bin.toolu == "dist/cli.js"' \
    "$ROOT/tools/toolu-cli/npm/package.json" >/dev/null
}

@test "every published package carries a README and a LICENSE" {
  for p in packages/toolu-core tools/toolu-opencode tools/toolu-cli/npm; do
    [ -f "$ROOT/$p/README.md" ] || { echo "$p has no README.md"; return 1; }
    [ -f "$ROOT/$p/LICENSE" ] || { echo "$p has no LICENSE"; return 1; }
  done
}

# npm acknowledges a publish minutes before the packument stops returning 404.
# The first ever @toolu/cli (6.8.1) was 404 for about six minutes after
# "+ @toolu/cli@6.8.1", so `npx @toolu/cli` failed right after the release went
# green. Green must mean installable.
@test "the workflow waits until every published package resolves before going green" {
  grep -Fq 'resolves on the registry' "$WF"
  grep -Fq 'is still not fetchable from the registry' "$WF"
  # The wait follows the publish loop; it is not a pre-publish check.
  publish_line=$(grep -n '^      - name: Publish$' "$WF" | cut -d: -f1)
  wait_line=$(grep -n '^      - name: Wait until every package resolves' "$WF" | cut -d: -f1)
  [ -n "$publish_line" ] && [ -n "$wait_line" ] && [ "$wait_line" -gt "$publish_line" ]
  # The wait is bounded and the job timeout leaves room for it.
  grep -Fq 'deadline=$(( $(date +%s) + 8 * 60 ))' "$WF"
  grep -Fq 'timeout-minutes: 20' "$WF"
  # setup-node's .npmrc reads NODE_AUTH_TOKEN on every npm call, so the wait
  # step needs the token too, not only the publish step.
  run bash -c "grep -c 'NODE_AUTH_TOKEN: \${{ secrets.NPM_TOKEN }}' '$WF'"
  [ "$output" = "2" ]
}
