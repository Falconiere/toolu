#!/usr/bin/env bats
# Tests for /comemory:setup (scripts/setup.sh) against the REAL comemory binary
# (no mocks for the READY path). The binary-state branches (MISSING/OLD/ERROR)
# are driven through the COMEMORY override seam — a stub or a bogus path — so
# coreutils stay on PATH and no package manager is ever invoked.

SETUP_SH="${BATS_TEST_DIRNAME}/../setup.sh"

# A tiny git repo with one indexable file + commit, plus an isolated data dir,
# so the READY path's install-hooks/index-code never touch the real store or a
# worktree (where `.git` is a file and install-hooks errors).
_make_repo() {
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"
  git init -q "$REPO"
  git -C "$REPO" config user.email t@t.t
  git -C "$REPO" config user.name t
  printf 'fn main() {}\n' > "$REPO/main.rs"
  git -C "$REPO" add -A
  git -C "$REPO" commit -q -m init
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/data"
  mkdir -p "$COMEMORY_DATA_DIR"
}

# Write a fake `comemory` whose --version prints $1 (so we can force OLD/ERROR
# without an old real binary). Echoes the path to the stub.
_version_stub() {
  local ver="$1" dir="$BATS_TEST_TMPDIR/vstub-$BATS_TEST_NUMBER"
  mkdir -p "$dir"
  printf '#!/bin/sh\ncase "$1" in --version) echo "%s";; *) exit 0;; esac\n' "$ver" > "$dir/comemory"
  chmod +x "$dir/comemory"
  printf '%s\n' "$dir/comemory"
}

@test "setup: -h prints usage and exits 0" {
  run bash "$SETUP_SH" -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: setup.sh"* ]]
}

@test "setup: MISSING when the binary is absent — prints the brew-tap install hint, exits 0" {
  run env COMEMORY=/nonexistent/comemory bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "MISSING" ]
  [[ "$output" == *"brew install Falconiere/tap/comemory"* ]]
  # never recommends the crates.io path that does not exist
  ! printf '%s\n' "$output" | grep -q 'cargo install comemory'
}

@test "setup: OLD when the binary is below the floor — prints the brew upgrade hint" {
  local stub
  stub=$(_version_stub "comemory 0.1.0")
  run env COMEMORY="$stub" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "OLD" ]
  [[ "$output" == *"brew upgrade Falconiere/tap/comemory"* ]]
}

@test "setup: ERROR when the version string is unparseable — exits non-zero" {
  local stub
  stub=$(_version_stub "garbage-no-version")
  run env COMEMORY="$stub" bash "$SETUP_SH"
  [ "$status" -ne 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "ERROR" ]
}

@test "setup: READY on the real binary wires install-hooks + index-code in a real repo" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  _make_repo
  cd "$REPO"
  run bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
  [[ "$output" == *"install-hooks: OK"* ]]
  [[ "$output" == *"index-code: OK"* ]]
  # The git hook really landed.
  [ -f "$REPO/.git/hooks/post-commit" ]
}

@test "setup: READY is idempotent — a second run still succeeds and re-reports" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  _make_repo
  cd "$REPO"
  run bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  # Second run: install-hooks now refuses (hooks exist) but setup stays exit 0.
  run bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
}

@test "setup: READY outside a git repo skips hooks/index but still succeeds" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/data-nogit"
  mkdir -p "$COMEMORY_DATA_DIR"
  cd "$BATS_TEST_TMPDIR"   # a fresh temp dir — not a git repo
  run bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
  [[ "$output" == *"not in a git repo"* ]]
  [[ "$output" == *"install-hooks: skipped (not a git repo)"* ]]
  [[ "$output" == *"index-code: skipped (not a git repo)"* ]]
}

@test "setup: --force re-runs cleanly and overwrites existing hooks" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  _make_repo
  cd "$REPO"
  run bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  run bash "$SETUP_SH" --force
  [ "$status" -eq 0 ]
  [[ "$output" == *"install-hooks: OK"* ]]
}

# Drive the READY path through the version stub (>= floor) so the marker step
# runs without depending on a real binary; TOOLU_PROJECT_DIR points the writer
# at the temp repo. jq is required to assert/merge the marker.
@test "setup: READY writes the comemory.setup_done marker into the repo config" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  local stub
  stub=$(_version_stub "comemory 9.9.9")
  _make_repo
  run env COMEMORY="$stub" TOOLU_PROJECT_DIR="$REPO" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
  [[ "$output" == *"memory: enabled"* ]]
  local cfg="$REPO/.claude/toolu.config.json"
  [ -f "$cfg" ]
  [ "$(jq -r '.comemory.setup_done' "$cfg")" = "true" ]
}

@test "setup: READY marker preserves pre-existing config keys" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  local stub
  stub=$(_version_stub "comemory 9.9.9")
  _make_repo
  mkdir -p "$REPO/.claude"
  printf '%s\n' '{"skills":{"ast-grep":false}}' > "$REPO/.claude/toolu.config.json"
  run env COMEMORY="$stub" TOOLU_PROJECT_DIR="$REPO" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  local cfg="$REPO/.claude/toolu.config.json"
  # both the new marker and the untouched prior key survive the jq merge.
  [ "$(jq -r '.comemory.setup_done' "$cfg")" = "true" ]
  [ "$(jq -r '.skills["ast-grep"]' "$cfg")" = "false" ]
}

@test "setup: READY with a malformed config WARNs the specific cause, leaves the file untouched, still exits 0" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  local stub
  stub=$(_version_stub "comemory 9.9.9")
  _make_repo
  mkdir -p "$REPO/.claude"
  local cfg="$REPO/.claude/toolu.config.json"
  # Pre-existing INVALID JSON (trailing comma + unquoted key) at the target path.
  local bad='{"skills": {"ast-grep": false,}, broken}'
  printf '%s\n' "$bad" > "$cfg"
  run env COMEMORY="$stub" TOOLU_PROJECT_DIR="$REPO" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "READY" ]
  # Specific, non-fatal WARN naming the real cause.
  [[ "$output" == *"is not valid JSON — fix it, then re-run /comemory:setup"* ]]
  # No marker was written (the merge was skipped, not generically failed).
  [[ "$output" != *"memory: enabled"* ]]
  # The user's file is byte-for-byte untouched — no data loss, no overwrite.
  [ "$(cat "$cfg")" = "$bad" ]
}

@test "setup: MISSING still runs NO package manager (no brew/curl install)" {
  # A bogus path drives MISSING; brew/curl shimmed to fail-loud so any actual
  # invocation would abort the run (proving the detect+guide stance holds).
  local shim="$BATS_TEST_TMPDIR/shim"
  mkdir -p "$shim"
  printf '#!/bin/sh\necho "RAN PACKAGE MANAGER: $0 $*" >&2\nexit 99\n' > "$shim/brew"
  printf '#!/bin/sh\necho "RAN PACKAGE MANAGER: $0 $*" >&2\nexit 99\n' > "$shim/curl"
  chmod +x "$shim/brew" "$shim/curl"
  run env COMEMORY=/nonexistent/comemory PATH="$shim:$PATH" bash "$SETUP_SH"
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "MISSING" ]
  ! printf '%s\n' "$output" | grep -q 'RAN PACKAGE MANAGER'
}
