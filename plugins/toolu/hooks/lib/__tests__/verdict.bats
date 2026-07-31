#!/usr/bin/env bats
# Tests for hooks/lib/verdict.sh — the unified read-only gate view.
#
# Real temp git repos and real state files: gate_record_failure (gate-file.sh)
# for the quality fixture, the real plan-ledger.sh CLI for the ledger fixture,
# hand-written v2 push-review JSON matching the spec schema, and a docs-sync
# attestation written at the exact path docs-sync.sh itself resolves. No
# mocked commands.
#
# TOOLU_CONFIG_DIR is pinned to an empty temp dir on every invocation so the
# machine's real ~/.claude/toolu.config.json (docsSync.mode, etc.) can never
# leak into a test's result.

LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
SCRIPT="$LIB_DIR/verdict.sh"
GATE_FILE_LIB="$LIB_DIR/gate-file.sh"
PLAN_LEDGER_CLI="$LIB_DIR/plan-ledger.sh"

setup() {
  TMP=$(mktemp -d)
  CFG_DIR="$TMP/cfg"
  mkdir -p "$CFG_DIR"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# build_repo DIR -> standalone temp git repo, base branch "main", one commit
# (so detect_base_branch's remote-less "main" fallback is the real base).
build_repo() {
  local dir="$1"
  mkdir -p "$dir"
  (
    cd "$dir" || exit 1
    git init -q -b main
    git config user.email "t@example.com"
    git config user.name "Tester"
    echo base > base.txt
    git add base.txt
    git commit -q -m base
  )
}

# raw_diff_sha REPO BASE -> the same formula toolu_diff_sha uses, for building
# fixtures independent of sourcing verdict.sh itself.
raw_diff_sha() {
  git -C "$1" diff --no-color "${2}...HEAD" | git hash-object --stdin
}

# write_review_state REPO SHA FILES_JSON [ROUND] [FINDINGS]
# Write a v2 push-review state file for REPO's current branch.
write_review_state() {
  local repo="$1" sha="$2" files_json="$3" round="${4:-1}" findings="${5:-0}"
  local branch slug state_dir
  branch=$(git -C "$repo" rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [ -n "$slug" ] || slug="_default"
  state_dir="$repo/.claude/tmp/push-review"
  mkdir -p "$state_dir"
  jq -n --arg branch "$branch" --arg sha "$sha" --argjson files "$files_json" \
    --argjson round "$round" --argjson findings "$findings" '{
      version: 2, branch: $branch, diff_sha: $sha, base_branch: "main",
      reviewed_at: "2026-07-30T00:00:00Z", reviewers: ["code-review"],
      findings_count: $findings, findings: [], review_round: $round,
      reviewed_files: $files
    }' > "$state_dir/${slug}.json"
}

# --- parity ---------------------------------------------------------------

@test "parity: verdict's ACCEPTED_REVIEWERS matches push-review.sh's accepted_reviewers literal" {
  # verdict.sh implements the review gate's schema independently of
  # pre-tools/modules/push-review.sh (see the file header) — the reviewer
  # allow-list is duplicated, not shared, so this is what keeps the two lists
  # from drifting apart silently (mirrors plan-ledger-model.bats's alias-list
  # parity test).
  local push_review="$LIB_DIR/../pre-tools/modules/push-review.sh"
  [ -f "$push_review" ]
  local from_verdict from_gate
  from_verdict=$(grep -o "ACCEPTED_REVIEWERS='\[[^]]*\]'" "$SCRIPT" \
    | sed -E "s/^ACCEPTED_REVIEWERS='(.*)'\$/\1/")
  from_gate=$(grep -o "accepted_reviewers='\[[^]]*\]'" "$push_review" \
    | sed -E "s/^accepted_reviewers='(.*)'\$/\1/")
  [ -n "$from_verdict" ]
  [ -n "$from_gate" ]
  [ "$from_verdict" = "$from_gate" ]
}

# --- AC-1: quality gate failure -----------------------------------------

@test "AC-1: recorded quality-gate failure -> blocked, exit 1, status renders" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  mkdir -p "$REPO/.claude/tmp"
  # shellcheck source=../gate-file.sh
  source "$GATE_FILE_LIB"
  gate_record_failure "$REPO/.claude/tmp/quality-gate-status.json" "/p/a.ts" "ts-quality-hook" "bad ts" "viol\n"

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  [ "$status" -eq 1 ]
  echo "$output" | jq -e '.gates.quality.state == "fail"'
  echo "$output" | jq -e '.overall == "blocked"'

  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" status
  [ "$status" -eq 1 ]
  [[ "$output" == *"quality"* ]]
  [[ "$output" == *"fail"* ]]
  [[ "$output" == *"overall: blocked"* ]]
}

# --- AC-2: everything green ----------------------------------------------

@test "AC-2: passing quality + fresh-green ledger + valid v2 review + doc touched -> ready, exit 0" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/example
    mkdir -p lib docs
    echo "echo hi" > lib/foo.sh
    echo "# doc" > docs/foo.md
    git add lib/foo.sh docs/foo.md
    git commit -q -m "add foo"
  )

  sha=$(raw_diff_sha "$REPO" main)
  write_review_state "$REPO" "$sha" '["docs/foo.md","lib/foo.sh"]'

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

  # Run the REAL plan-ledger CLI LAST so its stamped diff_sha matches cur.
  ( cd "$REPO" && TOOLU_CONFIG_DIR="$CFG_DIR" bash "$PLAN_LEDGER_CLI" run "$doc" )

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.overall == "ready"'
  echo "$output" | jq -e '.gates.quality.state == "pass"'
  echo "$output" | jq -e '.gates.plan.state == "pass"'
  echo "$output" | jq -e '.gates.plan.summary.total == 1 and .gates.plan.summary.fresh_green == 1'
  echo "$output" | jq -e '.gates.review.state == "pass"'
  echo "$output" | jq -e '.gates.docs.state == "pass"'
}

# --- AC-13: edge mapping ---------------------------------------------------

@test "AC-13: linked worktree -> quality skip" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  WT="$TMP/wt"
  ( cd "$REPO" && git worktree add -q -b feat/wt "$WT" main )

  cd "$WT"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.quality.state == "skip"'
  echo "$output" | jq -e '.gates.quality.reason | test("worktree")'
}

@test "AC-13: plan-less branch with code diff -> plan advise, exit 0" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/nocode
    mkdir -p lib
    echo "echo hi" > lib/foo.sh
    git add lib/foo.sh
    git commit -q -m "add foo.sh"
  )
  sha=$(raw_diff_sha "$REPO" main)
  write_review_state "$REPO" "$sha" '["lib/foo.sh"]'

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.gates.plan.state == "advise"'
  echo "$output" | jq -e '.overall == "ready"'
}

@test "AC-13: review_round=6 -> escalate, exit 1" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/round6
    echo "echo hi" > script.sh
    git add script.sh
    git commit -q -m "add script"
  )
  sha=$(raw_diff_sha "$REPO" main)
  write_review_state "$REPO" "$sha" '["script.sh"]' 6

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  [ "$status" -eq 1 ]
  echo "$output" | jq -e '.gates.review.state == "escalate"'
  echo "$output" | jq -e '.gates.review.reason_code == "round-cap"'
}

@test "AC-13: empty diff against base -> review reason_code empty-diff" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  ( cd "$REPO" && git checkout -q -b feat/empty )

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.review.state == "fail"'
  echo "$output" | jq -e '.gates.review.reason_code == "empty-diff"'
}

@test "AC-13: unparseable (present-but-garbage) push-review state -> reason_code schema, not no-state" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/garbage
    echo "echo hi" > script.sh
    git add script.sh
    git commit -q -m "add script"
  )
  mkdir -p "$REPO/.claude/tmp/push-review"
  echo "not json" > "$REPO/.claude/tmp/push-review/feat_garbage.json"

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.review.state == "fail"'
  echo "$output" | jq -e '.gates.review.reason_code == "schema"'
}

@test "AC-13: push-review state version 1 -> reason_code schema-v1" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/v1
    echo "echo hi" > script.sh
    git add script.sh
    git commit -q -m "add script"
  )
  sha=$(raw_diff_sha "$REPO" main)
  mkdir -p "$REPO/.claude/tmp/push-review"
  jq -n --arg sha "$sha" '{
    version: 1, branch: "feat/v1", diff_sha: $sha, base_branch: "main",
    reviewed_at: "2026-07-30T00:00:00Z", reviewers: ["code-review"],
    findings_count: 0, findings: []
  }' > "$REPO/.claude/tmp/push-review/feat_v1.json"

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.review.state == "fail"'
  echo "$output" | jq -e '.gates.review.reason_code == "schema-v1"'
}

@test "AC-13: push-review state version 3 -> reason_code schema (not schema-v1)" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/v3
    echo "echo hi" > script.sh
    git add script.sh
    git commit -q -m "add script"
  )
  sha=$(raw_diff_sha "$REPO" main)
  mkdir -p "$REPO/.claude/tmp/push-review"
  jq -n --arg sha "$sha" '{
    version: 3, branch: "feat/v3", diff_sha: $sha, base_branch: "main",
    reviewed_at: "2026-07-30T00:00:00Z", reviewers: ["code-review"],
    findings_count: 0, findings: [], reviewed_files: ["script.sh"]
  }' > "$REPO/.claude/tmp/push-review/feat_v3.json"

  cd "$REPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.review.state == "fail"'
  echo "$output" | jq -e '.gates.review.reason_code == "schema"'
}

@test "AC-13: docs attestation found via docs-sync's own path resolution (CLAUDE_PROJECT_DIR, worktree)" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  WT="$TMP/wt"
  ( cd "$REPO" && git worktree add -q -b feat/docs "$WT" main )
  (
    cd "$WT" || exit 1
    mkdir -p lib
    echo "echo hi" > lib/foo.sh
    git add lib/foo.sh
    git commit -q -m "add foo.sh"
  )

  sha=$(raw_diff_sha "$WT" main)
  # A CLAUDE_PROJECT_DIR distinct from BOTH $WT (the worktree/git-toplevel) and
  # $REPO (the main checkout) — proves verdict follows docs-sync's own path
  # formula, not its own repo_root.
  PROJECT_DIR="$TMP/claude-project-dir"
  mkdir -p "$PROJECT_DIR/.claude/tmp/docs-sync"
  jq -n --arg sha "$sha" '{
    version: 1, branch: "feat/docs", diff_sha: $sha, base_branch: "main",
    decision: "not-needed", note: "covered by test",
    attested_at: "2026-07-30T00:00:00Z"
  }' > "$PROJECT_DIR/.claude/tmp/docs-sync/feat_docs.json"

  cd "$WT"
  TOOLU_CONFIG_DIR="$CFG_DIR" CLAUDE_PROJECT_DIR="$PROJECT_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.docs.state == "pass"'
  echo "$output" | jq -e '.gates.docs.reason == "doc change attested as not needed"'
}

@test "docs attestation: CLAUDE_PROJECT_DIR unset -> writer and verdict both fall back to \$(pwd) and agree" {
  REPO="$TMP/repo"
  build_repo "$REPO"
  (
    cd "$REPO" || exit 1
    git checkout -q -b feat/nopd
    mkdir -p lib
    echo "echo hi" > lib/foo.sh
    git add lib/foo.sh
    git commit -q -m "add foo.sh"
  )

  sha=$(raw_diff_sha "$REPO" main)

  # Writer: same fallback formula docs-sync.sh/verdict.sh use —
  # ${DOCS_SYNC_STATE_DIR:-${CLAUDE_PROJECT_DIR:-$(pwd)}/.claude/tmp/docs-sync}
  # — with CLAUDE_PROJECT_DIR unset and cwd=$REPO, that's $REPO/.claude/tmp/docs-sync.
  (
    cd "$REPO" || exit 1
    unset CLAUDE_PROJECT_DIR
    mkdir -p "$(pwd)/.claude/tmp/docs-sync"
    jq -n --arg sha "$sha" '{
      version: 1, branch: "feat/nopd", diff_sha: $sha, base_branch: "main",
      decision: "not-needed", note: "covered by test",
      attested_at: "2026-07-30T00:00:00Z"
    }' > "$(pwd)/.claude/tmp/docs-sync/feat_nopd.json"
  )

  cd "$REPO"
  unset CLAUDE_PROJECT_DIR
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  echo "$output" | jq -e '.gates.docs.state == "pass"'
  echo "$output" | jq -e '.gates.docs.reason == "doc change attested as not needed"'
}

# --- exit 2: error cases -----------------------------------------------

@test "exit 2: cwd is not a git repository" {
  NOTREPO="$TMP/notrepo"
  mkdir -p "$NOTREPO"
  cd "$NOTREPO"
  TOOLU_CONFIG_DIR="$CFG_DIR" run bash "$SCRIPT" json
  [ "$status" -eq 2 ]
}

@test "exit 2: jq stripped from PATH" {
  STUB="$TMP/stub"
  mkdir -p "$STUB"
  for c in bash git; do
    src=$(command -v "$c") && ln -s "$src" "$STUB/$c"
  done
  run env -i HOME="$HOME" PATH="$STUB" bash "$SCRIPT" json
  [ "$status" -eq 2 ]
}
