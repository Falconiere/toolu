#!/usr/bin/env bats
# Tests for the telemetry write sites instrumented in:
#   - pre-tools/modules/push-review.sh (push_check)
#   - pre-tools/modules/docs-sync.sh   (docs_nudge / docs_attested)
#   - pre-tools/modules/plan-ledger.sh (ac_coverage)
# Real git sandboxes, real state/ledger files, no mocked commands. Deliberately
# self-contained (not `load`ing helpers.bash / docs-sync-helpers.bash, whose
# same-named functions target different HOOK_SCRIPTs and would collide if both
# were loaded into one file).

# Four levels up from modules/__tests__/ is the dir that CONTAINS hooks/ —
# plugins/toolu (mirrors helpers.bash / docs-sync-helpers.bash).
REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"
PUSH_REVIEW_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/push-review.sh"
DOCS_SYNC_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/docs-sync.sh"
PLAN_LEDGER_GATE_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/plan-ledger.sh"

setup() {
  SANDBOX="$(mktemp -d)"
  export SANDBOX
  export STATE_DIR="$SANDBOX/.claude/tmp/push-review"
  export DOCS_STATE_DIR="$SANDBOX/.claude/tmp/docs-sync"
  export LEDGER_DIR="$SANDBOX/.claude/tmp/plan-ledger"
  mkdir -p "$STATE_DIR" "$DOCS_STATE_DIR" "$LEDGER_DIR"
  TELEMETRY_FILE="$SANDBOX/.claude/tmp/telemetry/feat_example.jsonl"

  # Isolated config dirs so telemetry defaults enabled regardless of the host
  # machine's real ~/.claude config (mirrors lib/__tests__/telemetry.bats).
  export HOME="$SANDBOX/home"
  export EMPTY_CFG_DIR="$SANDBOX/agent"
  export TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR"
  export TOOLU_PROJECT_DIR="$SANDBOX"
  mkdir -p "$HOME/.claude" "$EMPTY_CFG_DIR"

  cd "$SANDBOX" || return 1
  git init -q -b development .
  git config user.email "test@example.com"
  git config user.name "Test"
  echo "base" > base.txt
  git add base.txt
  git commit -q -m "base commit"
  git checkout -q -b feat/example
}

teardown() {
  [ -n "${SANDBOX:-}" ] && rm -rf "$SANDBOX"
}

build_input() {
  jq -n --arg cmd "$1" '{ tool_name: "Bash", tool_input: { command: $cmd } }'
}

current_diff_sha() {
  git diff --no-color "development...HEAD" | git hash-object --stdin
}

commit_file() {
  local path="$1" body="${2:-x}"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$body" > "$path"
  git add "$path"
  git commit -q -m "add $path"
}

run_push_review() {
  local payload="$1"
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$PUSH_REVIEW_SCRIPT" <<<"$payload"
}

# reviewed_files is computed from the real diff at call time (not from $sha,
# which some callers set to a deliberately stale/bogus value) so it stays
# accurate regardless of which check a given test means to exercise.
write_push_state() {
  local sha="$1" count="$2" round="${3:-1}" branch slug reviewed_files
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  reviewed_files=$(git diff --no-color "development...HEAD" --name-only \
    | jq -R -s -c 'split("\n") | map(select(length > 0))')
  jq -n --arg branch "$branch" --arg sha "$sha" --argjson count "$count" --argjson round "$round" \
    --argjson reviewed_files "$reviewed_files" '{
    version: 2, branch: $branch, diff_sha: $sha, base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z", reviewers: ["code-review"],
    findings_count: $count, review_round: $round, findings: [],
    reviewed_files: $reviewed_files
  }' > "$STATE_DIR/${slug}.json"
}

run_docs_sync() {
  local payload="$1"
  tool_name="Bash" input="$payload" DOCS_SYNC_BASE=development DOCS_SYNC_STATE_DIR="$DOCS_STATE_DIR" \
    run bash "$DOCS_SYNC_SCRIPT" <<<"$payload"
}

write_docs_attestation() {
  local sha="$1" decision="${2:-not-needed}" branch slug
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" --arg d "$decision" --arg b "$branch" '{
    version: 1, branch: $b, diff_sha: $sha, base_branch: "development",
    decision: $d, note: "covered by test", attested_at: "2026-06-15T00:00:00Z"
  }' > "$DOCS_STATE_DIR/${slug}.json"
}

run_plan_ledger_gate() {
  local payload="$1"
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$PLAN_LEDGER_GATE_SCRIPT" <<<"$payload"
}

# --- push-review.sh: push_check ---------------------------------------------

@test "push-review push_check: no state file denies with reason_code=no-state, round=null" {
  commit_file feature.txt "feature"
  payload=$(build_input "git push")
  run_push_review "$payload"
  [ "$status" -eq 0 ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "push_check" ]
  [ "$(jq -r '.result' "$TELEMETRY_FILE")" = "deny" ]
  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "no-state" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "null" ]
}

@test "push-review push_check: clean push allows with reason_code=pass and the state's round" {
  commit_file feature.txt "feature"
  sha=$(current_diff_sha)
  write_push_state "$sha" 0 3
  payload=$(build_input "git push")
  run_push_review "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.result' "$TELEMETRY_FILE")" = "allow" ]
  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "pass" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "3" ]
}

@test "push-review push_check: open findings denies with reason_code=findings and the round" {
  commit_file feature.txt "feature"
  sha=$(current_diff_sha)
  write_push_state "$sha" 2 1
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "findings" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "1" ]
}

@test "push-review push_check: round-cap escalation denies with reason_code=round-cap" {
  commit_file feature.txt "feature"
  sha=$(current_diff_sha)
  write_push_state "$sha" 0 6
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "round-cap" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "6" ]
}

@test "push-review push_check: base branch missing locally denies with reason_code=base-missing, round=null" {
  commit_file feature.txt "feature"
  payload=$(build_input "git push")
  tool_name="Bash" input="$payload" PUSH_REVIEW_BASE=nonexistent-base \
    run bash "$PUSH_REVIEW_SCRIPT" <<<"$payload"
  [ "$status" -eq 0 ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.result' "$TELEMETRY_FILE")" = "deny" ]
  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "base-missing" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "null" ]
}

@test "push-review push_check: git diff failure (corrupted objects) allows with reason_code=diff-failed, round=null" {
  commit_file feature.txt "feature"
  # Corrupt the object store AFTER the refs exist: `rev-parse --verify` on the
  # base branch still resolves (ref lookup only) while `git diff` — which needs
  # real commit/tree/blob objects — fails for real. No mocked commands.
  rm -rf .git/objects
  mkdir -p .git/objects
  payload=$(build_input "git push")
  run_push_review "$payload"
  [ "$status" -eq 0 ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.result' "$TELEMETRY_FILE")" = "allow" ]
  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "diff-failed" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "null" ]
}

@test "push-review push_check: empty diff denies with reason_code=empty-diff, round=null" {
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "empty-diff" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "null" ]
}

@test "push-review push_check: stale diff denies with reason_code=stale-diff and the round" {
  commit_file feature.txt "feature"
  write_push_state "stale0000000000000000000000000000000000" 0 2
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "stale-diff" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "2" ]
}

@test "push-review push_check: no accepted reviewer denies with reason_code=reviewer and the round" {
  commit_file feature.txt "feature"
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 2, branch: "feat/example", diff_sha: $sha, base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z", reviewers: ["simplify"],
    findings_count: 0, review_round: 4, findings: [], reviewed_files: ["feature.txt"]
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "reviewer" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "4" ]
}

@test "push-review push_check: corrupted state file denies with reason_code=schema, round=null" {
  commit_file feature.txt "feature"
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  echo "not json" > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "schema" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "null" ]
}

@test "push-review push_check: reviewed_files short of the diff denies with reason_code=file-coverage and the round" {
  commit_file feature.txt "feature"
  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" '{
    version: 2, branch: "feat/example", diff_sha: $sha, base_branch: "development",
    reviewed_at: "2026-06-07T00:00:00Z", reviewers: ["code-review"],
    findings_count: 0, review_round: 2, findings: [], reviewed_files: []
  }' > "$STATE_DIR/${slug}.json"
  payload=$(build_input "git push")
  run_push_review "$payload"

  [ "$(jq -r '.reason_code' "$TELEMETRY_FILE")" = "file-coverage" ]
  [ "$(jq -r '.round' "$TELEMETRY_FILE")" = "2" ]
}

# --- docs-sync.sh: docs_nudge / docs_attested -------------------------------

@test "docs-sync docs_nudge: code change without a doc surface fires the advisory event" {
  commit_file lib/foo.sh "echo hi"
  payload=$(build_input "git push")
  run_docs_sync "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("docs-sync")'

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "docs_nudge" ]
}

@test "docs-sync docs_attested: a matching attestation fires docs_attested with its decision" {
  commit_file lib/foo.sh "echo hi"
  write_docs_attestation "$(current_diff_sha)" "not-needed"
  payload=$(build_input "git push")
  run_docs_sync "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "docs_attested" ]
  [ "$(jq -r '.decision' "$TELEMETRY_FILE")" = "not-needed" ]
}

# --- plan-ledger.sh (gate module): ac_coverage ------------------------------

_write_ac_spec() {
  cat > "$1" <<'EOF'
# Some Spec

## Acceptance criteria

- **AC-1:** first criterion.
- **AC-2:** second criterion.
EOF
}

_write_ac_plan() {
  local plan="$1" spec="$2"
  cat > "$plan" <<EOF
# Fixture Plan

**Spec:** $spec

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "covers AC-1", "check": "true", "ac_refs": ["AC-1"] }
]
\`\`\`
EOF
}

@test "plan-ledger gate ac_coverage: records covered/uncovered counts from a real plan+spec+ledger fixture" {
  echo "echo hi" > script.sh
  git add script.sh
  git commit -q -m script

  spec="$SANDBOX/spec.md"
  plan="$SANDBOX/plan.md"
  _write_ac_spec "$spec"
  _write_ac_plan "$plan" "$spec"

  sha=$(current_diff_sha)
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  jq -n --arg sha "$sha" --arg branch "$branch" --arg plan "$plan" '{
    version: 1, branch: $branch, base_branch: "development", plan_doc: $plan,
    updated_at: "2026-06-14T12:00:00Z",
    summary: { total: 1 },
    steps: [ { id: "s1", title: "covers AC-1", check: "true", status: "green",
               exit_code: 0, diff_sha: $sha, ac_refs: ["AC-1"] } ]
  }' > "$LEDGER_DIR/${slug}.json"

  payload=$(build_input "git push origin feat/example")
  run_plan_ledger_gate "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "ac_coverage" ]
  [ "$(jq -r '.covered' "$TELEMETRY_FILE")" = "1" ]
  [ "$(jq -r '.uncovered' "$TELEMETRY_FILE")" = "1" ]
}
