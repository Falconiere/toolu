#!/usr/bin/env bash
# Package and verify the native toolu release layout on the actual runner OS.
set -euo pipefail
release_stage=
trap 'if [ -n "$release_stage" ]; then rm -rf -- "$release_stage"; fi' EXIT

usage() {
  echo 'usage: release-native.sh verify-tag TAG [--test-tag] | package OS ARCH BINARY DIST | verify-package TAG ARCHIVE SUMS DEST [--test-tag]' >&2
  exit 2
}

verify_tag() {
  local tag=$1
  TAG="$tag" TEST_TAG="${2:-}" python3 - <<'PY'
import json
import os
import re
from pathlib import Path
import tomllib

tag = os.environ['TAG']
root = Path.cwd()
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
test_tag = os.environ['TEST_TAG'] == '--test-tag' and re.fullmatch(
    rf'v{re.escape(version)}-test(?:\.[0-9A-Za-z-]+)*', tag) is not None
if tag != f'v{version}' and not test_tag:
    raise SystemExit(f'tag {tag} does not match workspace v{version}')
lock = tomllib.loads((root / 'Cargo.lock').read_text())
for package in lock['package']:
    if 'source' not in package and package['version'] != version:
        raise SystemExit(f"Cargo.lock {package['name']} version {package['version']} does not match {version}")
paths = [root / 'package.json', *root.glob('packages/*/package.json'),
         *root.glob('tools/*/package.json'), *root.glob('tools/*/npm/package.json'),
         *root.glob('plugins/*/.claude-plugin/plugin.json'),
         *root.glob('plugins/*/.codex-plugin/plugin.json')]
for path in paths:
    found = json.loads(path.read_text())['version']
    if found != version:
        raise SystemExit(f'{path.relative_to(root)} version {found} does not match {version}')
print(f'verified {tag} against workspace and {len(paths)} manifests')
PY
}

package() {
  local os=$1 arch=$2 binary=$3 dist=$4
  test -f "$binary" || { echo "missing binary: $binary" >&2; exit 1; }
  mkdir -p "$dist"
  release_stage=$(mktemp -d)
  install -m 0755 "$binary" "$release_stage/toolu"
  cp LICENSE "$release_stage/LICENSE"
  tar -czf "$dist/toolu-$os-$arch.tar.gz" -C "$release_stage" toolu LICENSE
}

verify_package() {
  local tag=$1 archive=$2 sums=$3 dest=$4 test_tag=${5:-}
  local name expected actual listing version expected_version
  name=$(basename "$archive")
  test -s "$archive" && test -s "$sums" || { echo 'archive or checksums missing' >&2; exit 1; }
  expected=$(awk -v name="$name" '$2 == name {print $1}' "$sums")
  test -n "$expected" || { echo "no checksum for $name" >&2; exit 1; }
  actual=$(shasum -a 256 "$archive")
  actual=${actual%% *}
  test "$actual" = "$expected" || { echo "checksum mismatch for $name" >&2; exit 1; }
  test "$(wc -c < "$archive")" -ge 1048576 || { echo "archive too small: $name" >&2; exit 1; }
  listing=$(tar -tzf "$archive")
  test "$listing" = $'toolu\nLICENSE' || { echo "wrong archive layout: $name" >&2; exit 1; }
  mkdir -p "$dest"
  tar -xzf "$archive" -C "$dest"
  test -f "$dest/toolu" && test ! -L "$dest/toolu" && test -x "$dest/toolu" || { echo "toolu is not an executable regular file: $name" >&2; exit 1; }
  test -f "$dest/LICENSE" && test ! -L "$dest/LICENSE" || { echo "LICENSE is not a regular file: $name" >&2; exit 1; }
  version=$("$dest/toolu" --version)
  expected_version=${tag#v}
  if [ "$test_tag" = --test-tag ]; then expected_version=${expected_version%%-test*}; fi
  test "$version" = "toolu $expected_version" || { echo "binary version $version differs from $tag" >&2; exit 1; }
  echo "verified $name ($version)"
}

case "${1:-}" in
  verify-tag)
    { test "$#" -eq 2 || { test "$#" -eq 3 && test "$3" = --test-tag; }; } || usage
    verify_tag "$2" "${3:-}"
    ;;
  package)
    test "$#" -eq 5 || usage
    package "$2" "$3" "$4" "$5"
    ;;
  verify-package)
    { test "$#" -eq 5 || { test "$#" -eq 6 && test "$6" = --test-tag; }; } || usage
    verify_package "$2" "$3" "$4" "$5" "${6:-}"
    ;;
  *) usage ;;
esac
