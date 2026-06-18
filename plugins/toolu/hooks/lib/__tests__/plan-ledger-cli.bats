#!/usr/bin/env bats
# Tests for hooks/lib/plan-ledger.sh — checker lib + CLI (run/status/--self-test).
# Real git sandbox, real shell-command checks (true/false/echo). No mocks.

bats_require_minimum_version 1.5.0

# Absolute path to the script under test, resolved once.
SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"

setup() {
  TMP=$(mktemp -d)
  REPO="$TMP/repo"
  mkdir -p "$REPO"
  (
    cd "$REPO"
    git init -b main -q
    git config user.email "t@example.com"
    git config user.name "Tester"
    echo base > base.txt
    git add base.txt
    git commit -qm "base"
    git checkout -q -b feat/x
    echo feature > feature.txt
    git add feature.txt
    git commit -qm "feature"
  )
  # Base resolution: env override so detect_base_branch isn't needed (no origin).
  export PUSH_REVIEW_BASE=main
  # Ledger path: <root>/.claude/tmp/plan-ledger/<slug>.json. Branch feat/x -> feat_x.
  LEDGER="$REPO/.claude/tmp/plan-ledger/feat_x.json"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Write a 2-step plan doc; checks are passed in so each test controls red/green.
# $1=doc path  $2=s1 check  $3=s2 check
_write_doc() {
  cat > "$1" <<EOF
# Fixture Plan

## Context

Prose.

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "First step", "check": "$2" },
  { "id": "s2", "title": "Second step", "check": "$3" }
]
\`\`\`
EOF
}

# crit1 — mechanical truth: true=green, false=red.
@test "run: true/false -> s1 green, s2 red, summary green=1, next=s2, exit 1" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"

  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  [ -f "$LEDGER" ]

  [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER")" = "green" ]
  [ "$(jq -r '.steps[] | select(.id=="s1") | .exit_code' "$LEDGER")" = "0" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "red" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .exit_code' "$LEDGER")" = "1" ]
  [ "$(jq -r '.summary.total' "$LEDGER")" = "2" ]
  [ "$(jq -r '.summary.green' "$LEDGER")" = "1" ]
  [ "$(jq -r '.summary.red' "$LEDGER")" = "1" ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "1" ]
  [ "$(jq -r '.next' "$LEDGER")" = "s2" ]
}

# crit2 — single-step update flips only s2; s1 entry untouched (last_run stable).
@test "run --step: flips only s2, leaves s1 last_run unchanged, next=null, exit 0" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  s1_before=$(jq -r '.steps[] | select(.id=="s1") | .last_run' "$LEDGER")

  # Flip s2's check to true, run only s2.
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 0 ]

  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "green" ]
  [ "$(jq -r '.summary.green' "$LEDGER")" = "2" ]
  [ "$(jq -r '.next' "$LEDGER")" = "null" ]
  s1_after=$(jq -r '.steps[] | select(.id=="s1") | .last_run' "$LEDGER")
  [ "$s1_before" = "$s1_after" ]
}

# crit3 — evidence captured from combined stdout+stderr.
@test "run: red step's evidence_tail contains a marker printed to stderr" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "echo MARKER123 >&2; false"

  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  tail=$(jq -r '.steps[] | select(.id=="s2") | .evidence_tail' "$LEDGER")
  [[ "$tail" == *MARKER123* ]]
}

# crit8 — parse failure is loud: no steps block -> exit 2, no ledger written.
@test "run: doc with no steps block -> exit 2, no ledger file" {
  doc="$REPO/plan.md"
  cat > "$doc" <<'EOF'
# Fixture Plan

## Context

No machine-readable steps here.
EOF

  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 2 ]
  [ ! -f "$LEDGER" ]
}

# crit9 — status: partial -> prints next, exit 1; all-fresh-green -> exit 0.
@test "status: partial ledger prints next and exits 1" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 1 ]
  [[ "$output" == *"next=s2"* ]]
}

@test "status: all-fresh-green ledger exits 0 with next=none" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  [[ "$output" == *"next=none"* ]]
}

@test "status: absent ledger -> exit 2" {
  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 2 ]
}

# crit11 — staleness: all-green, then change diff -> status stale (exit 1); re-run -> exit 0.
@test "staleness: code change makes green steps stale, re-run re-greens" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  # Alter base...HEAD diff so current diff_sha differs from the stamped one.
  # NB: a `--allow-empty` commit does NOT change the content-addressed diff_sha
  # (the base...HEAD diff is byte-identical), so make a real code change.
  ( cd "$REPO" && echo more >> feature.txt && git add feature.txt && git commit -q -m change )

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 1 ]
  # Both steps are now stale: status green but diff_sha != current.
  [ "$(jq -r '.summary.stale' "$LEDGER")" = "2" ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "0" ]

  # Re-run re-stamps the current diff_sha -> fresh-green again.
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "2" ]
}

# Atomicity — no leftover temp files after a run.
@test "run: leaves no .tmp.* file behind" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  run bash -c "ls -1 '$REPO'/.claude/tmp/plan-ledger/*.tmp.* 2>/dev/null"
  [ "$status" -ne 0 ]
  [ -z "$output" ]
}

# Self-test — inline fixture parse.
@test "--self-test: exits 0" {
  run bash "$SCRIPT" --self-test
  [ "$status" -eq 0 ]
}

# --- a4: running two-write, --activity, orphan self-heal, path/root ---

# AC#2 — running is observable mid-run: background a --step run whose check sleeps,
# poll the ledger for status==running + summary.running==1, then assert green after.
@test "run --step: running observable mid-run, then green (AC#2)" {
  doc="$REPO/plan.md"
  # s1 sleeps then succeeds; s2 succeeds. Seed both first so the ledger exists.
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  _write_doc "$doc" "sleep 1; true" "true"
  # Background the single-step run; capture its PID.
  ( cd "$REPO" && bash "$SCRIPT" run "$doc" --step s1 --activity "checking s1" ) &
  local run_pid=$!

  # Poll the ledger for the running state during the sleep (up to ~2s).
  local saw_running=0 i
  for i in $(seq 1 40); do
    if [ -f "$LEDGER" ] \
       && [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER" 2>/dev/null)" = "running" ]; then
      saw_running=1
      [ "$(jq -r '.steps[] | select(.id=="s1") | .started_at' "$LEDGER")" != "null" ]
      [ "$(jq -r '.steps[] | select(.id=="s1") | .activity' "$LEDGER")" = "checking s1" ]
      [ "$(jq -r '.summary.running' "$LEDGER")" = "1" ]
      break
    fi
    sleep 0.05
  done
  [ "$saw_running" -eq 1 ]

  wait "$run_pid"
  # After the run returns, s1 is green and nothing is running.
  [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER")" = "green" ]
  [ "$(jq -r '.summary.running' "$LEDGER")" = "0" ]
}

# AC#3 — orphaned running self-heals: hand-set s1 running with started_at 600s
# ago, run `status`, and it comes back pending.
@test "status: orphaned running (started_at 600s ago) self-heals to pending (AC#3)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  # Hand-set s1 to running with an old started_at (600s in the past, UTC ISO-8601).
  local old
  old=$(date -u -v-600S +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d '600 seconds ago' +%Y-%m-%dT%H:%M:%SZ)
  local patched
  patched=$(jq --arg old "$old" '
    .steps |= map(if .id=="s1"
      then .status="running" | .started_at=$old | .activity="stuck"
      else . end)' "$LEDGER")
  printf '%s\n' "$patched" > "$LEDGER"
  [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER")" = "running" ]

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER")" = "pending" ]
  [ "$(jq -r '.steps[] | select(.id=="s1") | .started_at' "$LEDGER")" = "null" ]
  [ "$(jq -r '.summary.running' "$LEDGER")" = "0" ]
}

# A fresh running step (recent started_at) is NOT healed by status.
@test "status: fresh running step (recent started_at) is preserved, not healed" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  local now
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  local patched
  patched=$(jq --arg now "$now" '
    .steps |= map(if .id=="s1" then .status="running" | .started_at=$now else . end)' "$LEDGER")
  printf '%s\n' "$patched" > "$LEDGER"

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER")" = "running" ]
  [ "$(jq -r '.summary.running' "$LEDGER")" = "1" ]
}

# AC#5 — path/root are the single truth.
@test "path: prints <root>/.claude/tmp/plan-ledger/<slug>.json" {
  run bash -c "cd '$REPO' && bash '$SCRIPT' path"
  [ "$status" -eq 0 ]
  # path is rooted at the git toplevel, which on macOS resolves /tmp -> /private/tmp;
  # compare the suffix (slug-keyed file) and the resolved root separately.
  [ "${output%/.claude/tmp/plan-ledger/feat_x.json}" != "$output" ]
  local root_out
  root_out="${output%/.claude/tmp/plan-ledger/feat_x.json}"
  [ "$(cd "$root_out" && pwd -P)" = "$(cd "$REPO" && pwd -P)" ]
}

@test "root: prints detect_project_root (the repo toplevel)" {
  run bash -c "cd '$REPO' && bash '$SCRIPT' root"
  [ "$status" -eq 0 ]
  # macOS /tmp is a symlink to /private/tmp; compare resolved paths.
  [ "$(cd "$output" && pwd -P)" = "$(cd "$REPO" && pwd -P)" ]
}

@test "run: --activity without --step is rejected (exit 2)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --activity foo"
  [ "$status" -eq 2 ]
}
