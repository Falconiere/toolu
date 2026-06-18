#!/usr/bin/env bats
# Tests for plugins/toolu/hooks/pre-tools/modules/plan-ledger.sh
#
# Real git sandbox (no mocks). Reuses push-review's helpers for setup_sandbox
# (real repo, base `development`, branch `feat/example`) + build_input. Sets its
# OWN GATE_SCRIPT and ledger STATE_DIR, writes ledger fixtures via inline jq, and
# computes the sandbox's live diff_sha to stamp "fresh" fixtures.

load helpers

# The gate module under test (helpers.HOOK_SCRIPT points at push-review.sh).
GATE_SCRIPT="$REPO_ROOT/hooks/pre-tools/modules/plan-ledger.sh"

setup() {
  setup_sandbox
  # Ledger lives under .claude/tmp/plan-ledger/ (sibling of push-review's dir).
  export LEDGER_DIR="$SANDBOX/.claude/tmp/plan-ledger"
  mkdir -p "$LEDGER_DIR"
}

teardown() {
  teardown_sandbox
}

# Invoke the gate with dispatcher env (tool_name/input) like helpers.run_hook,
# but against GATE_SCRIPT. PUSH_REVIEW_BASE pins the base to the fixture's.
run_gate() {
  local tool_name="$1"
  local payload="$2"
  tool_name="$tool_name" input="$payload" PUSH_REVIEW_BASE=development \
    run bash "$GATE_SCRIPT" <<<"$payload"
}

# Path to the current branch's ledger file.
ledger_path() {
  local branch slug
  branch=$(git rev-parse --abbrev-ref HEAD)
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [[ -z "$slug" ]] && slug="_default"
  echo "$LEDGER_DIR/${slug}.json"
}

# Compute the sandbox's live branch diff_sha (same formula as the gate).
sandbox_diff_sha() {
  git diff --no-color "development...HEAD" | git hash-object --stdin
}

# Write a ledger fixture for the current branch. Args:
#   $1 = version, $2 = the `steps` json array, $3 = total (for summary).
write_ledger() {
  local version="$1" steps="$2" total="$3" branch
  branch=$(git rev-parse --abbrev-ref HEAD)
  jq -n \
    --argjson version "$version" \
    --arg branch "$branch" \
    --argjson steps "$steps" \
    --argjson total "$total" \
    '{
      version: $version,
      branch: $branch,
      base_branch: "development",
      plan_doc: "docs/toolu/plans/2026-06-14-planning-hardness.md",
      updated_at: "2026-06-14T12:00:00Z",
      summary: { total: $total },
      steps: $steps
    }' > "$(ledger_path)"
}

# True iff $output denies (jq -e finds permissionDecision == "deny").
denies() {
  jq -e '.hookSpecificOutput.permissionDecision == "deny"' <<<"$output" >/dev/null 2>&1
}

@test "plan-ledger gate: non-push command (git status) allows silently" {
  payload=$(build_input "git status")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "plan-ledger gate: non-Bash tool allows silently" {
  payload=$(build_input "git push")
  run_gate "Edit" "$payload"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# crit4: no ledger, diff touches only a .txt file -> ALLOW, no deny JSON.
@test "plan-ledger gate (crit4): no ledger + only .txt diff allows silently" {
  # feature.txt is the only changed file in the fixture; add another .txt.
  echo "notes" > notes.txt && git add notes.txt && git commit -q -m "notes"
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
  [ -z "$output" ]
}

# crit13: no ledger, commit a .sh/.ts code file -> ALLOW but stderr advisory.
@test "plan-ledger gate (crit13): no ledger + code file allows with stderr advisory" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
  [[ "$output" == *"no plan ledger for this change"* ]]
}

# crit5: ledger with one `red` step -> DENY; reason has step id + `run: bash`.
@test "plan-ledger gate (crit5): ledger with a red step denies and names it" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local sha
  sha=$(sandbox_diff_sha)
  local steps
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first thing", check: "true", status: "red",
      exit_code: 1, diff_sha: $sha }
  ]')
  write_ledger 1 "$steps" 1
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  denies
  [[ "$output" == *"s1"* ]]
  [[ "$output" == *"red"* ]]
  [[ "$output" == *"run: bash"* ]]
}

# crit6: ledger all green with diff_sha == current -> ALLOW.
@test "plan-ledger gate (crit6): all fresh-green allows" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local sha
  sha=$(sandbox_diff_sha)
  local steps
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first", check: "true", status: "green",
      exit_code: 0, diff_sha: $sha },
    { id: "s2", title: "second", check: "true", status: "green",
      exit_code: 0, diff_sha: $sha }
  ]')
  write_ledger 1 "$steps" 2
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
}

# crit7: a ledger under a DIFFERENT branch slug does not affect current push.
@test "plan-ledger gate (crit7): other-branch ledger does not affect current push" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  # Write a RED ledger keyed to a different branch slug. Current branch has none.
  local sha
  sha=$(sandbox_diff_sha)
  jq -n --arg sha "$sha" '{
    version: 1, branch: "other/branch", base_branch: "development",
    plan_doc: "x", updated_at: "2026-06-14T12:00:00Z",
    summary: { total: 1 },
    steps: [ { id: "s1", title: "x", check: "true", status: "red",
               exit_code: 1, diff_sha: $sha } ]
  }' > "$LEDGER_DIR/other_branch.json"
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
  # Current branch has no ledger but touched code -> advisory, not deny.
  [[ "$output" == *"no plan ledger for this change"* ]]
}

# staleness: ledger all green but diff_sha != current -> DENY, marks `stale`.
@test "plan-ledger gate (staleness): green-but-stale step denies and marks stale" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local steps
  steps=$(jq -n '[
    { id: "s1", title: "first", check: "true", status: "green",
      exit_code: 0, diff_sha: "stale000" }
  ]')
  write_ledger 1 "$steps" 1
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  denies
  [[ "$output" == *"s1"* ]]
  [[ "$output" == *"stale"* ]]
}

# crit12: ledger with summary.total==0 / empty steps -> ALLOW (no-op).
@test "plan-ledger gate (crit12): empty-steps ledger is a no-op allow" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  write_ledger 1 "[]" 0
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
}

# bad version: ledger .version==2 -> DENY (fail closed).
@test "plan-ledger gate (bad version): version!=1 denies (fail closed)" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local sha steps
  sha=$(sandbox_diff_sha)
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first", check: "true", status: "green",
      exit_code: 0, diff_sha: $sha }
  ]')
  write_ledger 2 "$steps" 1
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  denies
}

# a5/AC#4: a `running` step (fresh started_at) is listed as a push blocker.
@test "plan-ledger gate (a5): a running step denies and is named as a blocker" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local sha steps
  sha=$(sandbox_diff_sha)
  # s1 green+fresh, s2 running (fresh started_at) -> the running step blocks push.
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first", check: "true", status: "green",
      started_at: null, activity: null, exit_code: 0, diff_sha: $sha },
    { id: "s2", title: "second", check: "true", status: "running",
      started_at: "2026-06-17T12:00:00Z", activity: "checking s2",
      exit_code: null, diff_sha: null }
  ]')
  write_ledger 1 "$steps" 2
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  denies
  [[ "$output" == *"s2"* ]]
  [[ "$output" == *"running"* ]]
}

# a5: a version-1 ledger whose every step is fresh-green ALLOWS (the running case
# above is what flips it to deny — this pins the positive control on v1 schema).
@test "plan-ledger gate (a5): version-1 all-fresh-green ledger allows" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  local sha steps
  sha=$(sandbox_diff_sha)
  steps=$(jq -n --arg sha "$sha" '[
    { id: "s1", title: "first", check: "true", status: "green",
      started_at: null, activity: null, exit_code: 0, diff_sha: $sha },
    { id: "s2", title: "second", check: "true", status: "green",
      started_at: null, activity: null, exit_code: 0, diff_sha: $sha }
  ]')
  write_ledger 1 "$steps" 2
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  ! denies
}

# unparseable ledger -> DENY (fail closed).
@test "plan-ledger gate (corrupt): unparseable ledger denies (fail closed)" {
  echo "echo hi" > script.sh && git add script.sh && git commit -q -m "script"
  printf 'not json {{{' > "$(ledger_path)"
  payload=$(build_input "git push origin feat/example")
  run_gate "Bash" "$payload"
  [ "$status" -eq 0 ]
  denies
}
