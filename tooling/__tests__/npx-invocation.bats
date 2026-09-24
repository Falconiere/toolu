#!/usr/bin/env bats
# `npx @toolu/plugins <verb>` must reach the registry from any directory, this
# repository included, with no @latest tag.
#
# npm exec checks the local project tree before the registry. When the root
# package or a declared workspace carries the requested name, npx treats it as
# installed and runs its bin from node_modules/.bin, which here does not exist:
# `sh: toolu: command not found`. The old @toolu/cli did exactly that from every
# toolu checkout, because tools/toolu-cli was a workspace under that name.
#
# So the CLI publishes from tools/toolu-cli/npm, a folder no workspace declares,
# and the workspace itself is private under another name.

# Physical path: Arborist reports a workspace twice when the project path and
# the workspace's realpath differ only by a symlink (macOS /var -> /private/var).
ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd -P)"
PUBLISH_DIR="$ROOT/tools/toolu-cli/npm"

# Tracked markdown only: gitignored scratch specs are not published docs.
# CHANGELOG.md is release-please history and stays as it was written.
tracked_markdown() {
  git -C "$ROOT" ls-files -z -- '*.md' ':!CHANGELOG.md'
}

# The same lookup libnpmexec runs before it decides to install: Arborist's
# loadActual over the project, then an inventory query by package name. Uses
# the Arborist that ships inside npm, so it cannot drift from what npx does.
# Prints the locations of every local package with that name, as JSON.
local_matches() {
  local arborist
  arborist="$(npm root -g)/npm/node_modules/@npmcli/arborist"
  node -e '
    const Arborist = require(process.argv[1]);
    new Arborist({ path: process.argv[2] }).loadActual().then((tree) => {
      const hits = [...tree.inventory.query("packageName", process.argv[3])];
      console.log(JSON.stringify(hits.map((node) => node.location)));
    });
  ' "$arborist" "$ROOT" "$1"
}

@test "npm sees no local package named @toolu/plugins, so npx goes to the registry" {
  # The probe must still find the dev workspace, or an empty answer proves nothing.
  run local_matches toolu-cli
  [ "$status" -eq 0 ]
  [ "$output" = '["tools/toolu-cli"]' ]

  run local_matches @toolu/plugins
  [ "$status" -eq 0 ]
  [ "$output" = "[]" ]
}

@test "the packed tarball runs through npx with the verb as the first argument" {
  local tgz version
  # Isolated cache, and no update notice mixed into the captured output.
  export npm_config_cache="$BATS_TEST_TMPDIR/npm-cache"
  export npm_config_update_notifier=false
  (cd "$PUBLISH_DIR" && npm pack --silent --pack-destination "$BATS_TEST_TMPDIR" >/dev/null)
  tgz=$(find "$BATS_TEST_TMPDIR" -maxdepth 1 -name 'toolu-plugins-*.tgz')
  [ -n "$tgz" ]
  version=$(jq -r .version "$PUBLISH_DIR/package.json")
  cd "$BATS_TEST_TMPDIR"
  # A bare path is executed as a command; the file: spec makes npx install the
  # tarball and pick its bin exactly as it would for the registry package.
  tgz="file:$tgz"

  run npx --yes "$tgz" --version
  [ "$status" -eq 0 ]
  [ "$output" = "$version" ]

  run npx --yes "$tgz" --help
  [ "$status" -eq 0 ]
  [ "${lines[0]}" = "npx @toolu/plugins <command> [options]" ]

  # No command at all prints the same help, but as a usage error.
  run npx --yes "$tgz"
  [ "$status" -eq 2 ]
  [ "${lines[0]}" = "npx @toolu/plugins <command> [options]" ]

  # The noun grammar of the old @toolu/cli is gone: `plugins` is not a verb,
  # and the error says what to type instead.
  run npx --yes "$tgz" plugins install
  [ "$status" -eq 2 ]
  [ "$output" = 'toolu: unknown command: plugins. The package name already says plugins: run `npx @toolu/plugins install`' ]
}

@test "documented npx invocations use the bare @toolu/plugins name" {
  local found offenders
  found=$(cd "$ROOT" && tracked_markdown |
    xargs -0 grep -noE 'npx( +-[[:alnum:]-]+)* +@toolu/(cli|plugins)[^[:space:]`]*' || true)
  # The pattern must still see the documented commands, or this test is vacuous.
  [ -n "$found" ]
  offenders=$(printf '%s\n' "$found" | grep -vE ':npx @toolu/plugins$' || true)
  if [ -n "$offenders" ]; then
    printf 'document npx @toolu/plugins <verb>, with no tag, version, or old name:\n%s\n' "$offenders" >&2
    return 1
  fi
}

@test "no documented npx invocation uses the unscoped toolu package" {
  local offenders
  offenders=$(cd "$ROOT" && tracked_markdown |
    xargs -0 grep -nE 'npx( +-[[:alnum:]-]+)* +toolu([[:space:]`]|$)' || true)
  if [ -n "$offenders" ]; then
    printf 'the unscoped toolu package does not exist on npm; use npx @toolu/plugins:\n%s\n' "$offenders" >&2
    return 1
  fi
}
