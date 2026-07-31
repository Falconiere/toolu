#!/usr/bin/env bats
# Tests for hooks/lib/diff-sha.sh — the shared toolu_diff_sha helper.
# Real temp git repos, no mocked commands.
#
# (a)/(b)/(c) exercise toolu_diff_sha directly. (d) drives the migrated call
# sites' own scripts as subprocesses (same technique as their existing bats
# suites in pre-tools/modules/__tests__/) to prove each site's observable
# behavior is unchanged post-migration. Only ONE shared fixture file
# (helpers.bash, used by both push-review.bats and plan-ledger.bats today) is
# `load`ed — docs-sync-helpers.bash defines same-named functions
# (setup_sandbox/build_input/run_hook/...) that would silently clobber it if
# both were loaded into one file, so the docs-sync parity case is self-
# contained inline instead.

source_lib() {
  # shellcheck disable=SC1091
  . "${BATS_TEST_DIRNAME}/../diff-sha.sh"
}

# Raw formula every call site used before migrating to toolu_diff_sha.
raw_diff_sha() {
  git -C "$1" diff --no-color "${2}...HEAD" | git hash-object --stdin
}

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -q -b main
    git config user.email "t@example.com"
    git config user.name "Tester"
    echo base > base.txt
    git add base.txt
    git commit -qm base
    git checkout -q -b feat/x
    echo feature > feature.txt
    git add feature.txt
    git commit -qm feature
  )
}

teardown() {
  rm -rf "$TMP"
}

# --- (a) byte-identical to the raw formula ----------------------------------

@test "toolu_diff_sha: byte-identical to the raw formula for a repo with changes" {
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "$(raw_diff_sha "$REPO" main)" ]
}

@test "toolu_diff_sha: byte-identical to the raw formula after commit --amend" {
  ( cd "$REPO" && git commit -q --amend --no-edit )
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "$(raw_diff_sha "$REPO" main)" ]
}

# --- (b) empty diff -----------------------------------------------------
#
# NOTE (deviation from the literal AC-11 prose, flagged rather than silently
# resolved): `git hash-object --stdin` on an empty diff stream succeeds and
# prints the well-known empty-blob sha (e69de29...) — a real, non-empty
# 40-char value, not "empty output". Spec component 1 is explicit that
# toolu_diff_sha does "NO sentinel mapping" and only fails on "git failure OR
# empty output"; treating the empty-blob sha as a failure would itself BE a
# sentinel mapping, and would break push-review.sh's own `== $EMPTY_BLOB_SHA`
# check (it needs to SEE that exact value to apply its own sentinel). So an
# empty diff is a SUCCESS here (exit 0, prints the empty-blob sha) — matching
# the pre-migration pl_diff_sha behavior exactly. AC-11's "returns non-zero on
# empty diff" holds at the CALL-SITE level instead (see the push-review parity
# test below, which still denies on an empty diff via its own sentinel) — its
# parenthetical ("call sites keep their local sentinel/allow behavior")
# supports this reading.

@test "toolu_diff_sha: an empty diff succeeds and prints the well-known empty-blob sha" {
  ( cd "$REPO" && git checkout -q main && git checkout -q -b feat/empty )
  source_lib
  run toolu_diff_sha "$REPO" main
  [ "$status" -eq 0 ]
  [ "$output" = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391" ]
}

# --- (c) nonexistent base ref: non-zero -------------------------------------

@test "toolu_diff_sha: non-zero on a nonexistent base ref" {
  source_lib
  run toolu_diff_sha "$REPO" does-not-exist
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

# --- (d) per-call-site parity ------------------------------------------------

# Shared push-review/plan-ledger-module fixture (both existing suites `load`
# this same file) — gives setup_sandbox/teardown_sandbox/build_input/run_hook/
# REPO_ROOT/HOOK_SCRIPT (push-review.sh).
. "${BATS_TEST_DIRNAME}/../../pre-tools/modules/__tests__/helpers.bash"

@test "parity (push-review.sh): empty diff against base still denied with sentinel reason" {
  setup_sandbox
  git checkout -q development
  git checkout -q -b feat/empty
  payload=$(build_input "git push")
  run_hook "Bash" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("empty")'
  teardown_sandbox
}

# plan-ledger.sh (pre-tools gate): a ledger stamped with the RAW-formula sha is
# still recognized fresh-green post-migration (proves current_diff_sha inside
# the migrated gate equals the raw formula).
GATE_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/plan-ledger.sh"

run_gate() {
  local tool_name="$1" payload="$2"
  tool_name="$tool_name" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$GATE_SCRIPT" <<<"$payload"
}

@test "parity (plan-ledger.sh gate): ledger stamped with raw-formula sha is fresh-green" {
  setup_sandbox
  LEDGER_DIR="$SANDBOX/.claude/tmp/plan-ledger"
  mkdir -p "$LEDGER_DIR"
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m script
  sha=$(raw_diff_sha "$SANDBOX" development)
  branch=$(git rev-parse --abbrev-ref HEAD)
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first", check: "true", status: "green",
      exit_code: 0, diff_sha: $sha }
  ]')
  jq -n --arg branch "$branch" --argjson steps "$steps" '{
    version: 1, branch: $branch, base_branch: "development",
    plan_doc: "docs/plan.md", updated_at: "2026-07-30T00:00:00Z",
    summary: { total: 1 }, steps: $steps
  }' > "$LEDGER_DIR/feat_example.json"
  payload=$(build_input "git push origin feat/example")
  LEDGER_DIR="$LEDGER_DIR" run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  teardown_sandbox
}

# docs-sync.sh: a fresh raw-formula-keyed attestation still silences the
# nudge. Self-contained (no `load docs-sync-helpers` — see file header).
@test "parity (docs-sync.sh): attestation keyed to the raw-formula sha still silences the nudge" {
  local sandbox cfg_dir hook_script state_dir sha payload
  sandbox=$(mktemp -d)
  cfg_dir="$sandbox/agent"
  hook_script="${BATS_TEST_DIRNAME}/../../pre-tools/modules/docs-sync.sh"
  state_dir="$sandbox/.claude/tmp/docs-sync"
  mkdir -p "$state_dir" "$cfg_dir"

  cd "$sandbox"
  git init -q -b development .
  git config user.email "t@example.com"
  git config user.name "Test"
  echo base > base.txt && git add base.txt && git commit -q -m base
  git checkout -q -b feat/example
  mkdir -p lib && echo "echo hi" > lib/foo.sh
  git add lib/foo.sh && git commit -q -m "add lib/foo.sh"

  sha=$(raw_diff_sha "$sandbox" development)
  jq -n --arg sha "$sha" '{
    version: 1, branch: "feat/example", diff_sha: $sha, base_branch: "development",
    decision: "not-needed", note: "covered by test",
    attested_at: "2026-07-30T00:00:00Z"
  }' > "$state_dir/feat_example.json"

  payload=$(jq -n --arg cmd "git push" '{tool_name:"Bash", tool_input:{command:$cmd}}')
  tool_name="Bash" input="$payload" \
    DOCS_SYNC_BASE=development DOCS_SYNC_STATE_DIR="$state_dir" \
    TOOLU_CONFIG_DIR="$cfg_dir" TOOLU_PROJECT_DIR="$sandbox" \
    run bash "$hook_script" <<<"$payload"

  [ "$status" -eq 0 ]
  [ -z "$output" ]
  rm -rf "$sandbox"
}

# plan-ledger.sh (lib CLI, pl_diff_sha): a full run/status round trip stamps
# and re-recognizes fresh-green using the shared helper under the hood.
@test "parity (lib plan-ledger.sh CLI): run then status agree the ledger is fresh-green" {
  SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"
  doc="$REPO/plan.md"
  cat > "$doc" <<'EOF'
# Fixture Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "First step", "check": "true" }
]
```
EOF
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  [[ "$output" == *"1/1 fresh-green"* ]]
}
