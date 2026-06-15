#!/usr/bin/env bash
# tooling/release.sh <major.minor.patch>
#
# Atomic version bump for the toolu marketplace. The auto-update gate only
# re-extracts a plugin when its plugin.json "version" changes, so a release that
# forgets to bump a changed plugin silently ships stale code to users. This
# script removes that manual step:
#
#   - package.json + the toolu plugin  -> <new-version>  (toolu anchors the
#     monorepo; release.yml's tag gate checks plugins/toolu against the git tag).
#   - every OTHER plugin whose files changed since the last v*.*.* tag -> a PATCH
#     bump of its own (independent) version. Unchanged plugins are left alone.
#
# Versions are edited in place by line (NOT via jq, which would reorder keys and
# restyle the manifests). RELEASE_ROOT overrides the repo root for tests.
set -euo pipefail

ROOT="${RELEASE_ROOT:-$(cd "${BASH_SOURCE%/*}/.." && pwd)}"
cd "$ROOT"

# Strict major.minor.patch (no pre-release/build/extra segments) — the bumper
# and the manifests assume exactly three numeric components.
is_semver() { [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }

NEW="${1:-}"
is_semver "$NEW" || { printf 'usage: release.sh <major.minor.patch>\n' >&2; exit 1; }

# Replace the "version" field of a JSON file in place, touching only that line.
set_version() {  # $1=file  $2=version
  local f="$1" v="$2"
  is_semver "$v" || { printf 'refusing to write non-semver version "%s" to %s\n' "$v" "$f" >&2; return 1; }
  grep -qE '^[[:space:]]*"version":' "$f" || { printf 'no version field in %s\n' "$f" >&2; return 1; }
  sed -E "s|^([[:space:]]*\"version\":[[:space:]]*\")[^\"]*(\")|\1${v}\2|" "$f" > "$f.tmp"
  mv "$f.tmp" "$f"
}

bump_patch() {  # $1=x.y.z -> x.y.(z+1); fails loudly on a malformed input
  local a b c IFS=.
  is_semver "$1" || { printf 'refusing to bump non-semver version "%s"\n' "$1" >&2; return 1; }
  read -r a b c <<<"$1"
  printf '%s.%s.%s' "$a" "$b" "$((c + 1))"
}

cur_version() {  # $1=manifest -> its "version" value (may be empty/malformed)
  grep -E '^[[:space:]]*"version":' "$1" | head -1 | sed -E 's|.*"version":[[:space:]]*"([^"]*)".*|\1|'
}

last_tag="$(git describe --tags --abbrev=0 --match 'v*.*.*' 2>/dev/null || true)"
[ -n "$last_tag" ] || printf 'no v*.*.* tag found — bumping every changed-or-not plugin\n' >&2

# Pass 1 — resolve every change and validate, writing NOTHING yet, so a single
# malformed manifest aborts the release before any file is touched (atomicity).
declare -a files=() versions=() labels=()
files+=(package.json);                                versions+=("$NEW"); labels+=("package.json")
files+=(plugins/toolu/.claude-plugin/plugin.json);    versions+=("$NEW"); labels+=("toolu")

for manifest in plugins/*/.claude-plugin/plugin.json; do
  name="$(basename "$(dirname "$(dirname "$manifest")")")"
  [ "$name" = toolu ] && continue
  dir="plugins/$name"
  # Skip plugins untouched since the last release (only when we have a baseline).
  if [ -n "$last_tag" ] && git diff --quiet "$last_tag" -- "$dir"; then
    continue
  fi
  cur="$(cur_version "$manifest")"
  next="$(bump_patch "$cur")" || { printf 'release.sh: %s has a malformed version "%s" — fix it first\n' "$manifest" "$cur" >&2; exit 1; }
  files+=("$manifest"); versions+=("$next"); labels+=("$name $cur -> $next")
done

# Pass 2 — apply. set_version re-validates each value defensively.
for i in "${!files[@]}"; do
  set_version "${files[$i]}" "${versions[$i]}"
  printf '%s\n' "${labels[$i]}"
done
