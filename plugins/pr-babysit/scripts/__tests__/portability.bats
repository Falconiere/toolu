#!/usr/bin/env bats
# Portability gate for the pr-babysit helper (AC-15): the scripts must run on
# macOS /bin/bash 3.2, so bash-4+ constructs are banned by grep, every
# entrypoint and lib declares its strictness, and — where a 3.2 bash exists —
# it parses every script and reproduces the reducer's output byte for byte.

# Resolved, not "__tests__/..": the exclusion below would otherwise match the
# whole tree through the literal __tests__ path segment.
SCRIPTS="$(cd "${BATS_TEST_DIRNAME}/.." && pwd -P)"
SNAP="${BATS_TEST_DIRNAME}/fixtures/snapshots"

setup() {
  TMP=$(mktemp -d)
  FILES=$(find "$SCRIPTS" -name '*.sh' -not -path '*/__tests__/*' | sort)
  [ -n "$FILES" ]
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

@test "no bash-4-only constructs in any helper script" {
  # mapfile/readarray, associative arrays, wait -n, case-modifying expansions,
  # |& shorthand, ${var@Q}-style transformations.
  for f in $FILES; do
    ! grep -nE '(^|[^a-zA-Z_])(mapfile|readarray)([^a-zA-Z_]|$)' "$f"
    ! grep -nE 'declare +-[a-zA-Z]*A' "$f"
    ! grep -nE 'wait +-n' "$f"
    ! grep -nE '\$\{[a-zA-Z_][a-zA-Z0-9_]*(\[[^]]*\])?(,,|\^\^|@[QEPAa])\}' "$f"
    ! grep -nE '\|&' "$f"
  done
}

@test "every entrypoint sets -euo pipefail; every lib documents that it is sourced" {
  for f in "$SCRIPTS"/collect-pr.sh "$SCRIPTS"/reduce-state.sh "$SCRIPTS"/babysit-tick.sh "$SCRIPTS"/reply-thread.sh "$SCRIPTS"/resolve-thread.sh "$SCRIPTS"/record.sh; do
    grep -q '^set -euo pipefail$' "$f"
    [ -x "$f" ]
    head -1 "$f" | grep -q '^#!/usr/bin/env bash$'
  done
  for f in "$SCRIPTS"/lib/*.sh; do
    grep -qi 'sourced' "$f"
  done
}

@test "the helper never reads a session-specific environment variable for repo, PR, home or root" {
  for f in $FILES; do
    ! grep -nE '\$\{?(CLAUDE_PLUGIN_ROOT|PLUGIN_ROOT|CLAUDE_PROJECT_DIR|CODEX_HOME|HOME)\b' "$f"
  done
  grep -q 'BASH_SOURCE\[0\]' "$SCRIPTS/lib/common.sh"
}

@test "no Python anywhere in the helper" {
  for f in $FILES; do
    ! grep -nE '(^|[^a-zA-Z_])python3?([^a-zA-Z_]|$)' "$f"
  done
}

@test "/bin/bash 3.2 (macOS) parses every script and reproduces the reducer output byte for byte" {
  [ -x /bin/bash ] || skip "no /bin/bash"
  /bin/bash --version | head -1 | grep -q 'version 3\.2' || skip "/bin/bash is not 3.2 ($(/bin/bash --version | head -1))"
  for f in $FILES; do
    /bin/bash -n "$f"
  done
  jq '.pr.state = "OPEN" | .pr.mergeable = "MERGEABLE"' "$SNAP/toolu-165.json" >"$TMP/open.json"
  /bin/bash "$SCRIPTS/reduce-state.sh" --snapshot "$TMP/open.json" --state "$TMP/none.json" --now 2026-09-19T12:00:00Z --state-out "$TMP/a.state.json" >"$TMP/a.json"
  bash "$SCRIPTS/reduce-state.sh" --snapshot "$TMP/open.json" --state "$TMP/none.json" --now 2026-09-19T12:00:00Z --state-out "$TMP/b.state.json" >"$TMP/b.json"
  cmp "$TMP/a.json" "$TMP/b.json"
  cmp "$TMP/a.state.json" "$TMP/b.state.json"
  # And a whole tick, lock and all, under 3.2.
  /bin/bash "$SCRIPTS/babysit-tick.sh" --repo Falconiere/toolu --pr 165 --state-file "$TMP/slot.json" --snapshot-in "$SNAP/toolu-165.json" --now 2026-09-19T12:00:00Z >"$TMP/tick.json"
  [ "$(jq -r .decision "$TMP/tick.json")" = escalate ]
  /bin/bash "$SCRIPTS/record.sh" round --state-file "$TMP/slot.json" --had-rejection false --fix-pushed >/dev/null
  [ "$(jq -r .pr.fixAttempts "$TMP/slot.json")" = 1 ]
}
