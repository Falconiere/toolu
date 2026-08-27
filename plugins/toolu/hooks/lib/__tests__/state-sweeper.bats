#!/usr/bin/env bats
# Tests for hooks/lib/state-sweeper.sh.
#
# Real repository, real branches, real merges — "is this branch gone or
# merged?" is a question only git can answer, so every fixture here is an
# actual branch in an actual repo rather than a name in a list.

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  cd "$REPO"
  git init -q -b main
  git config user.email t@t
  git config user.name t
  echo base > base.txt && git add base.txt && git commit -qm base

  export HOME="$TMP/home"
  export TOOLU_PROJECT_DIR="$REPO"
  export CLAUDE_PROJECT_DIR="$REPO"
  unset TOOLU_CONFIG_DIR CLAUDE_CONFIG_DIR TOOLU_HOST_OVERRIDE
  mkdir -p "$HOME/.claude" "$REPO/.claude"

  STATE="$REPO/.claude/tmp"
  mkdir -p "$STATE/push-review" "$STATE/plan-ledger" "$STATE/docs-sync" "$STATE/telemetry"

  git checkout -qb feat/current
  echo work > work.txt && git add work.txt && git commit -qm work

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../state-sweeper.sh
  . "$REPO_ROOT/hooks/lib/state-sweeper.sh"
  TOOLU_CFG_LOADED=0
  _TOOLU_HAS_JQ=""
  TOOLU_CFG_JSON='{}'
}

# A branch created at main is an ANCESTOR of main, so `--merged` counts it as
# merged. An unmerged fixture needs a commit of its own.
unmerged_branch() {
  git checkout -q -b "$1" main
  echo "$1" > "${1//\//_}.txt"
  git add -A && git commit -qm "$1"
  git checkout -q feat/current
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

gate_config() {
  printf '%s' "$1" > "$REPO/.claude/toolu.config.json"
  TOOLU_CFG_LOADED=0
}

state_file() {
  printf '{"version":2,"branch":"%s"}' "$1" > "$STATE/$2/$1.json"
}

# Backdate a file so the TTL check sees it as old.
age_file() {
  touch -d "3 days ago" "$1" 2>/dev/null || touch -A -7200 "$1"
}

@test "state for a branch that no longer exists is reclaimed" {
  state_file gone_branch push-review
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/push-review/gone_branch.json" ]
}

@test "state for a merged branch is reclaimed" {
  git checkout -qb feat/merged main
  echo m > m.txt && git add m.txt && git commit -qm m
  git checkout -q main && git merge -q --no-ff feat/merged -m "merge"
  git checkout -q feat/current
  state_file feat_merged plan-ledger
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/plan-ledger/feat_merged.json" ]
}

@test "state for a live unmerged branch survives inside the TTL" {
  unmerged_branch feat/other
  state_file feat_other docs-sync
  toolu_sweep_state "$REPO"
  [ -f "$STATE/docs-sync/feat_other.json" ]
}

@test "state for a live unmerged branch is reclaimed once past the TTL" {
  unmerged_branch feat/other
  state_file feat_other docs-sync
  age_file "$STATE/docs-sync/feat_other.json"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/docs-sync/feat_other.json" ]
}

@test "the current branch's state is never reclaimed, however old" {
  state_file feat_current push-review
  age_file "$STATE/push-review/feat_current.json"
  toolu_sweep_state "$REPO"
  [ -f "$STATE/push-review/feat_current.json" ]
}

@test "waiver and pending-waiver files follow their branch" {
  printf '{"version":1}' > "$STATE/push-review/gone_branch.waiver.json"
  printf '{"version":1}' > "$STATE/push-review/gone_branch.pending-waiver.json"
  printf '{"version":1}' > "$STATE/push-review/feat_current.waiver.json"
  age_file "$STATE/push-review/feat_current.waiver.json"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/push-review/gone_branch.waiver.json" ]
  [ ! -f "$STATE/push-review/gone_branch.pending-waiver.json" ]
  [ -f "$STATE/push-review/feat_current.waiver.json" ]
}

@test "a passing quality gate record is reclaimed" {
  printf '{"status":"passing"}' > "$STATE/quality-gate-status.json"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/quality-gate-status.json" ]
}

@test "a failing gate about a file that still exists survives, however old" {
  printf '{"status":"failing","file":"%s","entries":{"%s":{}}}' "$REPO/work.txt" "$REPO/work.txt" \
    > "$STATE/quality-gate-status.json"
  age_file "$STATE/quality-gate-status.json"
  toolu_sweep_state "$REPO"
  [ -f "$STATE/quality-gate-status.json" ]
}

@test "a failing gate about deleted files is reclaimed" {
  printf '{"status":"failing","file":"%s","entries":{"%s":{}}}' "$REPO/vanished.ts" "$REPO/vanished.ts" \
    > "$STATE/quality-gate-status.json"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/quality-gate-status.json" ]
}

@test "a failing gate with a global entry survives" {
  printf '{"status":"failing","file":"__global__","entries":{"__global__":{}}}' \
    > "$STATE/quality-gate-status.json"
  toolu_sweep_state "$REPO"
  [ -f "$STATE/quality-gate-status.json" ]
}

@test "telemetry keeps recent lines and drops old ones" {
  old=$(date -u -d "10 days ago" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -v-10d +%Y-%m-%dT%H:%M:%SZ)
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  printf '{"v":1,"t":"%s","event":"old"}\n{"v":1,"t":"%s","event":"new"}\n' "$old" "$now" \
    > "$STATE/telemetry/feat_current.jsonl"
  toolu_sweep_state "$REPO"
  [ "$(wc -l < "$STATE/telemetry/feat_current.jsonl" | tr -d ' ')" = "1" ]
  grep -q '"event":"new"' "$STATE/telemetry/feat_current.jsonl"
}

@test "a telemetry file with only old lines is removed" {
  old=$(date -u -d "10 days ago" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -v-10d +%Y-%m-%dT%H:%M:%SZ)
  printf '{"v":1,"t":"%s","event":"old"}\n' "$old" > "$STATE/telemetry/stale.jsonl"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/telemetry/stale.jsonl" ]
}

@test "a telemetry file that is not valid JSONL is left untouched" {
  printf 'not json at all\n' > "$STATE/telemetry/broken.jsonl"
  toolu_sweep_state "$REPO"
  [ -f "$STATE/telemetry/broken.jsonl" ]
  [ "$(cat "$STATE/telemetry/broken.jsonl")" = "not json at all" ]
}

@test "files the sweeper does not own are never touched" {
  printf 'scratch' > "$STATE/scratch.txt"
  printf 'sentinel' > "$STATE/.permissions-written"
  mkdir -p "$STATE/someone-else"
  printf 'x' > "$STATE/someone-else/data.json"
  age_file "$STATE/scratch.txt"
  toolu_sweep_state "$REPO"
  [ -f "$STATE/scratch.txt" ]
  [ -f "$STATE/.permissions-written" ]
  [ -f "$STATE/someone-else/data.json" ]
}

@test "gates.sweep false disables the sweep entirely" {
  gate_config '{"version":1,"gates":{"sweep":false}}'
  state_file gone_branch push-review
  toolu_sweep_state "$REPO"
  [ -f "$STATE/push-review/gone_branch.json" ]
}

@test "a custom TTL is honored" {
  unmerged_branch feat/other
  state_file feat_other push-review
  # 1h TTL with a file backdated 3 days: reclaimed where the 24h default also would be.
  gate_config '{"version":1,"gates":{"stateTtlHours":1}}'
  age_file "$STATE/push-review/feat_other.json"
  toolu_sweep_state "$REPO"
  [ ! -f "$STATE/push-review/feat_other.json" ]
}

@test "a huge TTL keeps state a shorter one would reclaim" {
  unmerged_branch feat/other
  state_file feat_other push-review
  age_file "$STATE/push-review/feat_other.json"
  gate_config '{"version":1,"gates":{"stateTtlHours":8760}}'
  toolu_sweep_state "$REPO"
  [ -f "$STATE/push-review/feat_other.json" ]
}

@test "a missing state root is not an error" {
  rm -rf "$STATE"
  run toolu_sweep_state "$REPO"
  [ "$status" -eq 0 ]
}

@test "an unreadable state dir warns but still returns 0" {
  state_file gone_branch push-review
  chmod 000 "$STATE/push-review"
  run toolu_sweep_state "$REPO"
  chmod 755 "$STATE/push-review"
  [ "$status" -eq 0 ]
}

@test "sweeping twice is idempotent" {
  state_file gone_branch push-review
  toolu_sweep_state "$REPO"
  run toolu_sweep_state "$REPO"
  [ "$status" -eq 0 ]
  [ ! -f "$STATE/push-review/gone_branch.json" ]
}
