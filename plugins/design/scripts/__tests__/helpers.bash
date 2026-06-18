#!/usr/bin/env bash
# Shared bats helpers for the design plugin's detect-stack tests.
#
# The detector (detect-stack.sh) is fully standalone — pure bash + grep/sed,
# uses jq only when present, and reads only the local fixture manifests. It
# needs no network and no sandbox, so this helper stays minimal: just the
# detector path, the fixtures root, and a thin `detect` wrapper around bats
# `run`.

# helpers.bash lives in scripts/__tests__/; the scripts dir is one level up.
SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DETECTOR="$SCRIPTS_DIR/detect-stack.sh"
FIXTURES="$BATS_TEST_DIRNAME/fixtures"

# detect [args...] — run the detector under bats `run`, capturing $status/$output.
detect() {
  run bash "$DETECTOR" "$@"
}
