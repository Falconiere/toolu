#!/usr/bin/env bats
# Tests for tooling/shellcheck.sh.
#
# The script's whole reason to exist is the second pass: concern fragments are
# partials of one assembled script, so linting them individually reports noise
# (no shebang, cross-fragment variables "unassigned"/"unused") while linting the
# assembled module reports what actually runs. These tests drive the real script
# against a real fixture tree — no stubbing of shellcheck.

setup() {
  command -v shellcheck >/dev/null 2>&1 || skip "shellcheck not installed"
  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
  TMP=$(mktemp -d)
  mkdir -p "$TMP/tooling" "$TMP/plugins/demo/hooks/concerns" "$TMP/benchmarks" "$TMP/.claude-plugin"
  cp "$REPO_ROOT/tooling/shellcheck.sh" "$TMP/tooling/shellcheck.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# A pair of fragments that is only coherent once concatenated: 00 assigns the
# variable and carries the shebang, 10 consumes it. Linted separately, 10 raises
# SC2148 (no shebang) and SC2154 (COUNT unassigned).
_split_fragments() {
  cat > "$TMP/plugins/demo/hooks/concerns/00-preamble.sh" <<'EOF'
#!/usr/bin/env bash
COUNT=3
EOF
  cat > "$TMP/plugins/demo/hooks/concerns/10-use.sh" <<'EOF'
if [ "$COUNT" -gt 2 ]; then
  printf 'many\n'
fi
EOF
}

@test "shellcheck.sh: passes on a clean tree" {
  _split_fragments
  cat > "$TMP/plugins/demo/ok.sh" <<'EOF'
#!/usr/bin/env bash
printf 'hello\n'
EOF
  run bash "$TMP/tooling/shellcheck.sh"
  [ "$status" -eq 0 ]
}

@test "shellcheck.sh: fragments are linted assembled, not individually" {
  # Guards the design: run the SAME fragments through plain per-file shellcheck
  # and assert it reports the noise this script exists to avoid, then assert the
  # script itself stays green on them.
  _split_fragments
  run shellcheck -S warning "$TMP/plugins/demo/hooks/concerns/10-use.sh"
  [ "$status" -ne 0 ]
  echo "$output" | grep -q "SC2148"

  run bash "$TMP/tooling/shellcheck.sh"
  [ "$status" -eq 0 ]
}

@test "shellcheck.sh: fails on a warning-level defect in a standalone script" {
  _split_fragments
  cat > "$TMP/plugins/demo/bad.sh" <<'EOF'
#!/usr/bin/env bash
# SC2206: unquoted expansion into an array.
list="a b c"
arr=($list)
printf '%s\n' "${arr[0]}"
EOF
  run bash "$TMP/tooling/shellcheck.sh"
  [ "$status" -ne 0 ]
  echo "$output" | grep -q "SC2206"
}

@test "shellcheck.sh: a concern defect is caught by the assembled pass" {
  # Concerns are excluded from the standalone pass, so the assembled pass is the
  # ONLY thing that can see a defect in a fragment. A variable assigned in one
  # fragment and consumed by no other is dead across the whole module (SC2034) —
  # exactly the cross-fragment question a per-file lint cannot answer.
  cat > "$TMP/plugins/demo/hooks/concerns/00-preamble.sh" <<'EOF'
#!/usr/bin/env bash
COUNT=3
NEVER_CONSUMED="dead"
EOF
  cat > "$TMP/plugins/demo/hooks/concerns/10-use.sh" <<'EOF'
if [ "$COUNT" -gt 2 ]; then
  printf 'many\n'
fi
EOF
  run bash "$TMP/tooling/shellcheck.sh"
  [ "$status" -ne 0 ]
  echo "$output" | grep -q "SC2034"
  echo "$output" | grep -q "NEVER_CONSUMED"
}

@test "shellcheck.sh: reports the missing binary instead of passing silently" {
  # env -i with a PATH holding only the interpreters the script needs, so
  # `command -v shellcheck` misses while bash/find/mktemp still resolve.
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash find mktemp basename dirname cat rm echo sort; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c" 2>/dev/null || true
  done
  run env -i PATH="$STUB" HOME="$TMP" bash "$TMP/tooling/shellcheck.sh"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q "shellcheck not on PATH"
}
