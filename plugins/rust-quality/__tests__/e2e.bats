#!/usr/bin/env bats
# End-to-end test of the deterministic refactor pipeline against a REAL,
# compilable, non-compliant Rust crate (spec
# docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md, AC-9 + AC-11).
#
# NO mocks: every test generates a fresh `git init` repo + a real crate in its
# own temp dir and drives the actual scripts and the actual cargo toolchain
# (fmt / clippy / nextest / check). The toolchain IS present in this environment,
# so the cargo-dependent tests MUST run for real — they `skip` cleanly only on a
# machine that lacks cargo / cargo-nextest, never here.
#
# AC-9  — a crate non-compliant ONLY in mechanically-fixable ways reaches green
#         (fmt-check, clippy -D warnings, nextest) after preflight + apply, the
#         scan's block-severity mechanical/config violations resolve to zero, and
#         a SECOND apply is a true no-op (no new commits, tree stays green).
# AC-11 — the `_apply_step` rollback helper reverts a step that breaks
#         `cargo check` (working tree restored, recorded as a `manual` residual)
#         and commits a step that keeps the build compiling.

SCRIPTS="${BATS_TEST_DIRNAME}/../scripts"
PREFLIGHT="$SCRIPTS/rust-refactor-preflight.sh"
APPLY="$SCRIPTS/rust-refactor-apply.sh"
SCAN="$SCRIPTS/rust-scan.sh"

setup() {
  TMP="$(mktemp -d)"
  REPO="$TMP/crate"
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Skip the whole test cleanly if a required real tool is absent. In THIS
# environment everything is present, so this never fires here.
_need() {
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || skip "missing required tool: $tool"
  done
}

# Build a real, compilable single-crate workspace that is non-compliant ONLY in
# mechanically-fixable ways:
#   - badly formatted source (whitespace, no spaces around ops/params)
#   - a `cargo clippy --fix`-able lint (needless_return)
#   - NO rustfmt.toml / clippy.toml / deny.toml / [workspace.lints]
#   - a trivial passing test in a flat tests/ dir so nextest is green
# The source is clean of every scaffolded clippy DENY (no .unwrap(), no panic!,
# no unsafe, no indexing, no casts) so that `clippy -D warnings` passes once the
# deny lints are scaffolded in — the point is the pipeline reaches green, not
# that it auto-fixes every deny. A module //! doc is present so the only scan
# residual left is the advisory "doc lacks a # Public API section", which is a
# structural class an agent owns (reported, never asserted zero).
_make_fixable_crate() {
  mkdir -p "$REPO/src" "$REPO/tests"

  cat > "$REPO/Cargo.toml" <<'EOF'
[package]
name = "e2e_fixture"
version = "0.1.0"
edition = "2024"

[dependencies]
EOF

  # A real crate ignores its build dir — without this the apply pipeline's
  # `git add -A` would sweep target/ into commits and the second apply would
  # never be a no-op.
  printf '/target\n' > "$REPO/.gitignore"

  # Badly formatted + a needless_return clippy --fix can repair.
  printf '%s' '//! Fixture crate library.
pub fn add(   a: u32,b :u32 )->u32{
    return a+b;
}
' > "$REPO/src/lib.rs"

  # Trivial passing integration test in a flat tests/ dir -> nextest green.
  printf '%s' '//! Integration test.
#[test]
fn adds() {
    assert_eq!(e2e_fixture::add(2, 3), 5);
}
' > "$REPO/tests/basic.rs"

  ( cd "$REPO" \
    && git init -q \
    && git config user.email e2e@toolu.test \
    && git config user.name e2e \
    && git add -A \
    && git commit -q -m "initial fixture" )
}

# Build a minimal real crate (single lib, no test deps) on the refactor branch,
# for driving _apply_step directly. Clean + compiling at HEAD.
_make_rollback_crate() {
  mkdir -p "$REPO/src"
  cat > "$REPO/Cargo.toml" <<'EOF'
[package]
name = "rollback_fixture"
version = "0.1.0"
edition = "2024"

[dependencies]
EOF
  printf '/target\n' > "$REPO/.gitignore"
  printf '%s' '//! Rollback fixture.
pub fn answer() -> u32 {
    42
}
' > "$REPO/src/lib.rs"
  ( cd "$REPO" \
    && git init -q \
    && git config user.email e2e@toolu.test \
    && git config user.name e2e \
    && git add -A \
    && git commit -q -m "initial" \
    && git checkout -q -b rust-quality/refactor )
}

# ── AC-9: mechanical compliance -> green + idempotent ──────────────────────

@test "AC-9: preflight + apply takes a mechanically-fixable crate to green, idempotently" {
  _need git cargo rustfmt jq
  command -v cargo-nextest >/dev/null 2>&1 || skip "missing cargo-nextest"

  _make_fixable_crate

  # Sanity: the crate really IS non-compliant before the pipeline runs —
  # fmt-check fails and none of the four config artifacts exist yet.
  run bash -c "cd '$REPO' && cargo fmt --all -- --check"
  [ "$status" -ne 0 ]
  [ ! -f "$REPO/rustfmt.toml" ]
  [ ! -f "$REPO/clippy.toml" ]
  [ ! -f "$REPO/deny.toml" ]
  run grep -qE '^\s*\[workspace\.lints' "$REPO/Cargo.toml"
  [ "$status" -ne 0 ]

  # Preflight: clean tree + Cargo workspace -> creates and checks out the branch.
  run bash "$PREFLIGHT" --path "$REPO"
  [ "$status" -eq 0 ]
  run bash -c "git -C '$REPO' rev-parse --abbrev-ref HEAD"
  [ "$output" = "rust-quality/refactor" ]

  # Apply pass 1 — the mechanical pipeline (scaffold -> fmt -> clippy --fix).
  run bash "$APPLY" --path "$REPO"
  [ "$status" -eq 0 ]

  commits_after_first="$(git -C "$REPO" rev-list --count HEAD)"
  head_after_first="$(git -C "$REPO" rev-parse HEAD)"

  # The tree is clean (every mechanical step committed or was a no-op).
  run bash -c "git -C '$REPO' status --porcelain"
  [ -z "$output" ]

  # The four config artifacts now exist (scaffolded).
  [ -f "$REPO/rustfmt.toml" ]
  [ -f "$REPO/clippy.toml" ]
  [ -f "$REPO/deny.toml" ]
  run grep -qE '^\s*\[workspace\.lints' "$REPO/Cargo.toml"
  [ "$status" -eq 0 ]

  # The three real green gates.
  run bash -c "cd '$REPO' && cargo fmt --all -- --check"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && cargo clippy --all-targets -- -D warnings"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && cargo nextest run"
  [ "$status" -eq 0 ]

  # Re-scan: the mechanically-fixable violation classes are resolved.
  #   - every config class is now present (config.missing is empty)
  #   - no BLOCK-severity violation remains (block-class total == 0)
  # Structural advisory residuals (agent moves) MAY remain and are only
  # reported, never asserted zero.
  scan_json="$(bash "$SCAN" --path "$REPO" --json 2>/dev/null)"
  [ -n "$scan_json" ]

  missing_count="$(printf '%s' "$scan_json" | jq '.config.missing | length')"
  [ "$missing_count" -eq 0 ]
  present_count="$(printf '%s' "$scan_json" | jq '.config.present | length')"
  [ "$present_count" -eq 4 ]

  block_total="$(printf '%s' "$scan_json" \
    | jq '[.crates[].violations[] | select(.severity == "block")] | length')"
  [ "$block_total" -eq 0 ]

  # Report (not assert) any structural residuals an agent still owns.
  residual="$(printf '%s' "$scan_json" \
    | jq -r '[.crates[].violations[]
              | "  residual [\(.severity)] \(.file) \(.rule) (\(.autofix))"] | .[]')"
  if [ -n "$residual" ]; then
    printf 'AC-9 structural residuals (expected, agent-owned):\n%s\n' "$residual" >&3
  fi

  # Apply pass 2 — a true no-op: no new commits, HEAD unchanged, tree clean,
  # and still green.
  run bash "$APPLY" --path "$REPO"
  [ "$status" -eq 0 ]

  commits_after_second="$(git -C "$REPO" rev-list --count HEAD)"
  head_after_second="$(git -C "$REPO" rev-parse HEAD)"
  [ "$commits_after_second" -eq "$commits_after_first" ]
  [ "$head_after_second" = "$head_after_first" ]

  run bash -c "git -C '$REPO' status --porcelain"
  [ -z "$output" ]

  run bash -c "cd '$REPO' && cargo fmt --all -- --check"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && cargo clippy --all-targets -- -D warnings"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && cargo nextest run"
  [ "$status" -eq 0 ]
}

# ── AC-11: rollback preserves compilation ──────────────────────────────────

@test "AC-11: _apply_step rolls back a build-breaking step and keeps a compiling one" {
  _need git cargo jq

  _make_rollback_crate

  # Drive the helper from a real bash subshell (bats IS bash, so BASH_SOURCE and
  # `source` work). Sourcing the apply script runs its mechanical pipeline once
  # on this clean crate (idempotent + harmless) and leaves `_apply_step` + $REPO
  # defined; then we call the helper directly for the break and ok cases. All
  # PASS/FAIL booleans are printed for the asserts below to grep verbatim.
  run bash -c '
    set +e
    REPO_DIR="'"$REPO"'"
    cd "$REPO_DIR"
    source "'"$APPLY"'" --path "$REPO_DIR" >/dev/null 2>&1
    type -t _apply_step >/dev/null || { echo "HELPER_MISSING"; exit 1; }

    AP_MANUAL_RESIDUALS=""

    # Baseline: the crate compiles before the break step.
    cargo check --all-targets >/dev/null 2>&1 \
      && echo "BASELINE=PASS" || echo "BASELINE=FAIL"

    commits_before="$(git rev-list --count HEAD)"

    # BREAK: a step that writes uncompilable Rust. _apply_step commits it, runs
    # cargo check (fails), then `git reset --hard HEAD~1`s it out and records the
    # label as a manual residual.
    _apply_step "break" bash -c "printf \"this is not valid rust @@@\n\" >> src/lib.rs" \
      >/dev/null 2>&1

    commits_after_break="$(git rev-list --count HEAD)"
    [ "$commits_after_break" = "$commits_before" ] \
      && echo "BREAK_RESET=PASS" || echo "BREAK_RESET=FAIL"

    [ -z "$(git status --porcelain)" ] \
      && echo "BREAK_TREE_CLEAN=PASS" || echo "BREAK_TREE_CLEAN=FAIL"

    # The working tree was restored to the pre-step COMPILING state.
    cargo check --all-targets >/dev/null 2>&1 \
      && echo "BREAK_CHECK_RESTORED=PASS" || echo "BREAK_CHECK_RESTORED=FAIL"

    printf "%s" "$AP_MANUAL_RESIDUALS" | grep -qx "break" \
      && echo "BREAK_RESIDUAL=PASS" || echo "BREAK_RESIDUAL=FAIL"

    # OK: a compiling no-op edit. _apply_step commits it (cargo check ok) and
    # does NOT record it as a residual.
    commits_before_ok="$(git rev-list --count HEAD)"
    _apply_step "ok" bash -c "printf \"// trailing comment\n\" >> src/lib.rs" \
      >/dev/null 2>&1
    commits_after_ok="$(git rev-list --count HEAD)"

    [ "$commits_after_ok" -eq $((commits_before_ok + 1)) ] \
      && echo "OK_COMMITTED=PASS" || echo "OK_COMMITTED=FAIL"

    cargo check --all-targets >/dev/null 2>&1 \
      && echo "OK_CHECK=PASS" || echo "OK_CHECK=FAIL"

    [ -z "$(git status --porcelain)" ] \
      && echo "OK_TREE_CLEAN=PASS" || echo "OK_TREE_CLEAN=FAIL"

    printf "%s" "$AP_MANUAL_RESIDUALS" | grep -qx "ok" \
      && echo "OK_RESIDUAL=PRESENT" || echo "OK_RESIDUAL=ABSENT"
  '
  [ "$status" -eq 0 ]

  # Baseline compiled, the break step rolled back, the ok step committed.
  [[ "$output" == *"BASELINE=PASS"* ]]
  [[ "$output" == *"BREAK_RESET=PASS"* ]]
  [[ "$output" == *"BREAK_TREE_CLEAN=PASS"* ]]
  [[ "$output" == *"BREAK_CHECK_RESTORED=PASS"* ]]
  [[ "$output" == *"BREAK_RESIDUAL=PASS"* ]]
  [[ "$output" == *"OK_COMMITTED=PASS"* ]]
  [[ "$output" == *"OK_CHECK=PASS"* ]]
  [[ "$output" == *"OK_TREE_CLEAN=PASS"* ]]
  [[ "$output" == *"OK_RESIDUAL=ABSENT"* ]]
}
