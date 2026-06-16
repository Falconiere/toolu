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

@test "release: --dry-run writes nothing" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" --dry-run 1.1.0
  [ "$(jq -r .version "$REPO/package.json")" = "1.0.0" ]
  [ "$(jq -r .version "$REPO/plugins/toolu/.claude-plugin/plugin.json")" = "1.0.0" ]
  [ "$(jq -r .version "$REPO/plugins/alpha/.claude-plugin/plugin.json")" = "0.1.0" ]
  [ "$(jq -r .version "$REPO/plugins/beta/.claude-plugin/plugin.json")" = "0.2.0" ]
  [ ! -e "$REPO/docs/releases/v1.1.0.md" ]
}

@test "release: --dry-run prints a plan with diff stats" {
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" --dry-run 1.1.0
  [ "$status" -eq 0 ]
  # Anchor transitions show up in the plan.
  [[ "$output" == *"1.0.0 -> 1.1.0"* ]]
  [[ "$output" == *"0.1.0 -> 0.1.1"* ]]
  # Per-plugin diff stat is included (alpha is the only changed plugin).
  [[ "$output" == *"file changed"* ]]
  # Unchanged plugins are listed in the "skipped" footer.
  [[ "$output" == *"skipped"* ]]
  [[ "$output" == *"beta"* ]]
  # Banner identifies dry-run.
  [[ "$output" == *"dry-run"* ]]
}

@test "release: apply auto-drafts docs/releases/v1.1.0.md" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ -f "$REPO/docs/releases/v1.1.0.md" ]
  # Header + release date.
  grep -qF '# toolu v1.1.0' "$REPO/docs/releases/v1.1.0.md"
  grep -qE '^Released: [0-9]{4}-[0-9]{2}-[0-9]{2}$' "$REPO/docs/releases/v1.1.0.md"
  # Per-plugin bullets: toolu (anchored) + alpha (changed) are present; beta (unchanged) is NOT.
  grep -qF '`toolu`' "$REPO/docs/releases/v1.1.0.md"
  grep -qF '`alpha`' "$REPO/docs/releases/v1.1.0.md"
  ! grep -qF '`beta`' "$REPO/docs/releases/v1.1.0.md"
  # Section header uses the exact "## Included changes since v<prev>" form.
  grep -qF '## Included changes since v1.0.0' "$REPO/docs/releases/v1.1.0.md"
  # Upgrade notes section is always drafted (matches tooling/templates/release-notes.md).
  grep -qF '## Upgrade notes' "$REPO/docs/releases/v1.1.0.md"
  # Highlights placeholder.
  grep -qF '## Highlights' "$REPO/docs/releases/v1.1.0.md"
  grep -qF 'TODO' "$REPO/docs/releases/v1.1.0.md"
  # toolu bullet shows the captured pre-bump version (1.0.0) — exercises the
  # toolu_old variable (not a hardcoded index into old_versions).
  grep -qF '(`toolu` 1.0.0 -> 1.1.0)' "$REPO/docs/releases/v1.1.0.md"
}

@test "release: apply refuses to overwrite an existing notes file (and aborts before printing the plan)" {
  mkdir -p "$REPO/docs/releases"
  printf 'marker\n' > "$REPO/docs/releases/v1.1.0.md"
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.1.0
  [ "$status" -ne 0 ]
  # Error message is on stderr — captured by bats' `run` as $output.
  [[ "$output" == *"already exists"* ]]
  # Pre-flight runs before the plan banner, so the plan must NOT appear.
  [[ "$output" != *"plan for v1.1.0"* ]]
  # Marker preserved verbatim.
  [ "$(cat "$REPO/docs/releases/v1.1.0.md")" = "marker" ]
  # Atomicity: no manifest was bumped.
  [ "$(jq -r .version "$REPO/package.json")" = "1.0.0" ]
  [ "$(jq -r .version "$REPO/plugins/toolu/.claude-plugin/plugin.json")" = "1.0.0" ]
  [ "$(jq -r .version "$REPO/plugins/alpha/.claude-plugin/plugin.json")" = "0.1.0" ]
}

@test "release: --no-notes skips the draft but still bumps" {
  RELEASE_ROOT="$REPO" bash "$SCRIPT" --no-notes 1.1.0
  [ ! -e "$REPO/docs/releases/v1.1.0.md" ]
  # Manifests are still bumped.
  [ "$(jq -r .version "$REPO/package.json")" = "1.1.0" ]
  [ "$(jq -r .version "$REPO/plugins/toolu/.claude-plugin/plugin.json")" = "1.1.0" ]
  [ "$(jq -r .version "$REPO/plugins/alpha/.claude-plugin/plugin.json")" = "0.1.1" ]
}

@test "release: no-prev-tag drafts without a 'since' clause and lists every plugin" {
  ( cd "$REPO" && git tag -d v1.0.0 )
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" 1.0.0
  [ "$status" -eq 0 ]
  [ -f "$REPO/docs/releases/v1.0.0.md" ]
  # No "since v<num>" anywhere.
  ! grep -qE 'since v[0-9]' "$REPO/docs/releases/v1.0.0.md"
  # Initial-release marker is in the placeholder.
  grep -qF '## Included changes' "$REPO/docs/releases/v1.0.0.md"
  grep -qF 'initial release' "$REPO/docs/releases/v1.0.0.md"
  # Every plugin (toolu, alpha, beta) is in the bullet list.
  grep -qF '`toolu`' "$REPO/docs/releases/v1.0.0.md"
  grep -qF '`alpha`' "$REPO/docs/releases/v1.0.0.md"
  grep -qF '`beta`' "$REPO/docs/releases/v1.0.0.md"
  # toolu was anchored to the requested version.
  [ "$(jq -r .version "$REPO/plugins/toolu/.claude-plugin/plugin.json")" = "1.0.0" ]
}

@test "release: --help exits 0 and prints usage" {
  run env RELEASE_ROOT="$REPO" bash "$SCRIPT" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"usage:"* ]]
  [[ "$output" == *"--dry-run"* ]]
  [[ "$output" == *"--no-notes"* ]]
}
