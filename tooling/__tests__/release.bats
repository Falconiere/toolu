#!/usr/bin/env bats
# Tests for tooling/release.sh against a REAL throwaway git repo (no mocks):
# a fixture marketplace with a v* tag, one plugin changed after the tag and one
# not, driven via the RELEASE_ROOT override.

SCRIPT="${BATS_TEST_DIRNAME}/../release.sh"

_ver() { jq -r .version "$REPO/$1"; }

setup() {
  REPO=$(mktemp -d)
  (
    cd "$REPO" || exit 1
    git init -q
    git config user.email a@b.c
    git config user.name t
    mkdir -p plugins/toolu/.claude-plugin plugins/alpha/.claude-plugin plugins/beta/.claude-plugin
    printf '{\n  "name": "toolu",\n  "version": "1.0.0"\n}\n' > package.json
    printf '{\n  "name": "toolu",\n  "version": "1.0.0"\n}\n' > plugins/toolu/.claude-plugin/plugin.json
    printf '{\n  "name": "alpha",\n  "version": "0.1.0"\n}\n' > plugins/alpha/.claude-plugin/plugin.json
    printf '{\n  "name": "beta",\n  "version": "0.2.0"\n}\n' > plugins/beta/.claude-plugin/plugin.json
    git add -A && git commit -qm init && git tag -m rel v1.0.0
    printf 'echo hi\n' > plugins/alpha/run.sh   # change ONLY alpha after the tag
    git add -A && git commit -qm "touch alpha"
  )
}

teardown() {
  [ -n "${REPO:-}" ] && [ -d "$REPO" ] && rm -rf "$REPO"
}

@test "release: anchors package.json + toolu to the new version" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ "$(jq -r .version "$REPO/package.json")" = "1.1.0" ]
  [ "$(_ver plugins/toolu/.claude-plugin/plugin.json)" = "1.1.0" ]
}

@test "release: patch-bumps a plugin changed since the last tag" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ "$(_ver plugins/alpha/.claude-plugin/plugin.json)" = "0.1.1" ]
}

@test "release: leaves an unchanged plugin untouched" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ "$(_ver plugins/beta/.claude-plugin/plugin.json)" = "0.2.0" ]
}

@test "release: rejects a non-semver argument" {
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" not-a-version
  [ "$status" -ne 0 ]
}

@test "release: rejects a loosely-formed version (extra segment)" {
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.2.3.4
  [ "$status" -ne 0 ]
}

@test "release: a malformed manifest version aborts without corrupting it" {
  # Make alpha (changed since the tag) carry a non-semver version.
  printf '{\n  "name": "alpha",\n  "version": "0.1.x"\n}\n' > "$REPO/plugins/alpha/.claude-plugin/plugin.json"
  ( cd "$REPO" && git commit -qam "break alpha version" )
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ "$status" -ne 0 ]
  # alpha untouched (no empty version written); no orphan tmp left behind.
  [ "$(jq -r .version "$REPO/plugins/alpha/.claude-plugin/plugin.json")" = "0.1.x" ]
  [ ! -f "$REPO/plugins/alpha/.claude-plugin/plugin.json.tmp" ]
  # Atomic: the anchor (package.json + toolu) must NOT have been bumped either.
  [ "$(jq -r .version "$REPO/package.json")" = "1.0.0" ]
  [ "$(jq -r .version "$REPO/plugins/toolu/.claude-plugin/plugin.json")" = "1.0.0" ]
}

@test "release: manifests stay valid JSON after the bump" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" 2.0.0
  for f in package.json plugins/toolu/.claude-plugin/plugin.json plugins/alpha/.claude-plugin/plugin.json; do
    jq -e . "$REPO/$f" >/dev/null
  done
}
