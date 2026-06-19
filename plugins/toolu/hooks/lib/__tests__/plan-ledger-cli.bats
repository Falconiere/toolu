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

# --- s3: enriched schema (ac_refs/depends_on/input/retries) at all sites ---

# A plan whose s1 carries all three authored fields; s2 is legacy {id,title,check}.
# Checks are passed in so each test controls red/green.  $1=doc $2=s1 check $3=s2 check
_write_enriched_doc() {
  cat > "$1" <<EOF
# Enriched Fixture Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "First step", "check": "$2",
    "ac_refs": ["AC-1", "AC-2"], "depends_on": ["s0"], "input": "fixture input" },
  { "id": "s2", "title": "Second step", "check": "$3" }
]
\`\`\`
EOF
}

# AC-2 — every step object carries ac_refs/depends_on/input/retries in green AND red
# states after a full run; authored values flow through, legacy step defaults.
@test "run: green + red steps carry ac_refs/depends_on/input/retries (AC-2)" {
  doc="$REPO/plan.md"
  _write_enriched_doc "$doc" "true" "false"

  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]

  # s1 (green) — authored values present, retries seeded to [].
  [ "$(jq -c '.steps[] | select(.id=="s1") | .ac_refs' "$LEDGER")" = '["AC-1","AC-2"]' ]
  [ "$(jq -c '.steps[] | select(.id=="s1") | .depends_on' "$LEDGER")" = '["s0"]' ]
  [ "$(jq -r '.steps[] | select(.id=="s1") | .input' "$LEDGER")" = "fixture input" ]
  [ "$(jq -c '.steps[] | select(.id=="s1") | .retries' "$LEDGER")" = '[]' ]

  # s2 (red, legacy) — defaults []/[]/null, retries [].
  [ "$(jq -c '.steps[] | select(.id=="s2") | .ac_refs' "$LEDGER")" = '[]' ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .depends_on' "$LEDGER")" = '[]' ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .input' "$LEDGER")" = "null" ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .retries' "$LEDGER")" = '[]' ]
}

# AC-2 — pending state (a --step run seeds the OTHER steps pending) carries the
# new keys too, proving the pending-seed serialization site stays in sync.
@test "run --step: untouched step seeded pending carries the new keys (AC-2)" {
  doc="$REPO/plan.md"
  # No prior ledger: a --step s1 run must seed s2 as pending with the new keys.
  _write_enriched_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s1"
  [ "$status" -eq 1 ]

  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "pending" ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .ac_refs' "$LEDGER")" = '[]' ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .depends_on' "$LEDGER")" = '[]' ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .input' "$LEDGER")" = "null" ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .retries' "$LEDGER")" = '[]' ]
}

# AC-2 — running state (the pre-write site) carries the new keys. Background a
# --step run whose check sleeps, poll the running ledger, assert the keys.
@test "run --step: running pre-write entry carries the new keys (AC-2)" {
  doc="$REPO/plan.md"
  _write_enriched_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  _write_enriched_doc "$doc" "sleep 1; true" "true"
  ( cd "$REPO" && bash "$SCRIPT" run "$doc" --step s1 --activity "checking s1" ) &
  local run_pid=$!

  local saw_running=0 i
  for i in $(seq 1 40); do
    if [ -f "$LEDGER" ] \
       && [ "$(jq -r '.steps[] | select(.id=="s1") | .status' "$LEDGER" 2>/dev/null)" = "running" ]; then
      saw_running=1
      [ "$(jq -c '.steps[] | select(.id=="s1") | .ac_refs' "$LEDGER")" = '["AC-1","AC-2"]' ]
      [ "$(jq -c '.steps[] | select(.id=="s1") | .depends_on' "$LEDGER")" = '["s0"]' ]
      [ "$(jq -r '.steps[] | select(.id=="s1") | .input' "$LEDGER")" = "fixture input" ]
      [ "$(jq -c '.steps[] | select(.id=="s1") | .retries' "$LEDGER")" = '[]' ]
      break
    fi
    sleep 0.05
  done
  [ "$saw_running" -eq 1 ]
  wait "$run_pid"
}

# summary.retried counts steps with non-empty retries (zero on a first run).
@test "run: summary.retried is 0 when no step has retries (AC-2)" {
  doc="$REPO/plan.md"
  _write_enriched_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  [ "$(jq -r '.summary.retried' "$LEDGER")" = "0" ]
}

# --- s4: retries[] archival across runs ---

# AC-3 — a step that runs red, then re-runs green (--step) after a fix, has exactly
# one retries entry capturing the prior red (exit_code/evidence_tail/at). A step
# green on first run has retries: []. summary.retried tracks it.
@test "run --step: red then green archives the prior red into retries (AC-3)" {
  doc="$REPO/plan.md"
  # s2 starts red and prints a marker we can find in the archived evidence.
  _write_doc "$doc" "true" "echo BOOM_S2 >&2; false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  # First run: green s1 has empty retries; red s2 also has empty retries (nothing
  # prior to archive yet).
  [ "$(jq -c '.steps[] | select(.id=="s1") | .retries' "$LEDGER")" = '[]' ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .retries' "$LEDGER")" = '[]' ]
  red_exit=$(jq -r '.steps[] | select(.id=="s2") | .exit_code' "$LEDGER")
  red_at=$(jq -r '.steps[] | select(.id=="s2") | .last_run' "$LEDGER")

  # Fix s2 and re-run only it.
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 0 ]

  # s2 is now green with exactly one archived retry capturing the prior red.
  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "green" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries | length' "$LEDGER")" = "1" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[0].attempt' "$LEDGER")" = "1" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[0].exit_code' "$LEDGER")" = "$red_exit" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[0].at' "$LEDGER")" = "$red_at" ]
  tail=$(jq -r '.steps[] | select(.id=="s2") | .retries[0].evidence_tail' "$LEDGER")
  [[ "$tail" == *BOOM_S2* ]]
  # s1 (green throughout, never archived) still has empty retries.
  [ "$(jq -c '.steps[] | select(.id=="s1") | .retries' "$LEDGER")" = '[]' ]
  [ "$(jq -r '.summary.retried' "$LEDGER")" = "1" ]
}

# AC-3 — two consecutive reds yield exactly one archived entry after the second
# run (each run archives the single prior red; attempt counter advances).
@test "run --step: two consecutive reds archive once, attempt advances on re-green (AC-3)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]

  # Second red run on s2: archives the first red -> one entry, still red.
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 1 ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "red" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries | length' "$LEDGER")" = "1" ]

  # Third run greens it: archives the second red -> two entries, attempts 1 then 2.
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries | length' "$LEDGER")" = "2" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[0].attempt' "$LEDGER")" = "1" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[1].attempt' "$LEDGER")" = "2" ]
}

# AC-9 — a FULL run (no --step) after a fix still preserves the prior red.
@test "run (full): full re-run after a fix preserves the prior red in retries (AC-9)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  red_at=$(jq -r '.steps[] | select(.id=="s2") | .last_run' "$LEDGER")

  # Fix s2; FULL run (no --step). The final full run must NOT wipe history.
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "green" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries | length' "$LEDGER")" -ge 1 ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries[0].at' "$LEDGER")" = "$red_at" ]
  # s1 was green on both full runs -> never archived.
  [ "$(jq -c '.steps[] | select(.id=="s1") | .retries' "$LEDGER")" = '[]' ]
}

# AC-3 — a `status` call between runs must NOT mutate retries (byte-identical).
@test "status: leaves retries byte-identical between runs (AC-3)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 0 ]

  before=$(jq -cS '.steps[] | select(.id=="s2") | .retries' "$LEDGER")
  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  after=$(jq -cS '.steps[] | select(.id=="s2") | .retries' "$LEDGER")
  [ "$before" = "$after" ]
}

# Fail-closed — a corrupt prior ledger makes `run` exit 2 (full and --step),
# rather than silently archiving nothing and clobbering it.
@test "run: corrupt prior ledger -> exit 2 (fail-closed), full run" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  mkdir -p "$(dirname "$LEDGER")"
  printf '%s\n' 'this is not json {' > "$LEDGER"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 2 ]
}

# Write a 2-step plan where each step carries an ac_refs array (passed in) so the
# authored field can be edited between runs. $1=doc $2=s1 ac_ref $3=s2 ac_ref.
# Both checks are "false" so a full run records both red (retries fodder).
_write_doc_acrefs() {
  cat > "$1" <<EOF
# AC-refs Fixture Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "First step", "check": "false", "ac_refs": ["$2"] },
  { "id": "s2", "title": "Second step", "check": "false", "ac_refs": ["$3"] }
]
\`\`\`
EOF
}

# Regression (s3/s4 SHOULD-FIX): a --step run must RE-DERIVE authored fields
# (ac_refs/depends_on/input) for NON-targeted carried-over steps from the current
# plan doc, not reuse the stale prior entry — while preserving engine state
# (retries/exit_code). Otherwise editing a plan's ac_refs and re-running a
# DIFFERENT step leaves the edited step's ledger ac_refs stale, which false-flags
# AC coverage in `status` until the next full run.
@test "run --step: non-targeted step's ac_refs refresh from the edited plan; retries/exit_code preserved" {
  doc="$REPO/plan.md"
  # Full run: both steps red (s2 references AC-OLD). This seeds prior reds.
  _write_doc_acrefs "$doc" "AC-1" "AC-OLD"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  [ "$(jq -c '.steps[] | select(.id=="s2") | .ac_refs' "$LEDGER")" = '["AC-OLD"]' ]
  s2_exit_before=$(jq -r '.steps[] | select(.id=="s2") | .exit_code' "$LEDGER")
  [ "$s2_exit_before" = "1" ]

  # Re-run s2 alone so it archives its own prior red into retries (engine state we
  # must later prove is preserved when s2 is the NON-targeted step).
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 1 ]
  s2_retries_before=$(jq -c '.steps[] | select(.id=="s2") | .retries' "$LEDGER")
  [ "$(jq -r '.steps[] | select(.id=="s2") | .retries | length' "$LEDGER")" = "1" ]

  # EDIT the plan: s2's ac_refs changes to AC-NEW. Then run a DIFFERENT step (s1).
  _write_doc_acrefs "$doc" "AC-1" "AC-NEW"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s1"
  [ "$status" -eq 1 ]

  # The non-targeted s2's ac_refs is REFRESHED to the new authored value...
  [ "$(jq -c '.steps[] | select(.id=="s2") | .ac_refs' "$LEDGER")" = '["AC-NEW"]' ]
  # ...while its engine state (retries + exit_code) is preserved unchanged.
  [ "$(jq -c '.steps[] | select(.id=="s2") | .retries' "$LEDGER")" = "$s2_retries_before" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .exit_code' "$LEDGER")" = "$s2_exit_before" ]
  [ "$(jq -r '.steps[] | select(.id=="s2") | .status' "$LEDGER")" = "red" ]
}

# --- s5: backward compatibility with a legacy v1 ledger (no new keys) ---

# Hand-write a legacy v1 ledger (the feat_plan-ledger-hardness.json shape) for the
# two-step fixture plan: step entries carry ONLY the pre-enrichment keys — no
# ac_refs/depends_on/input/retries. $1 = both steps' diff_sha (use the live one
# for fresh-green).
_write_legacy_ledger() {
  local sha="$1"
  mkdir -p "$(dirname "$LEDGER")"
  cat > "$LEDGER" <<EOF
{
  "version": 1,
  "branch": "feat/x",
  "base_branch": "main",
  "plan_doc": "$REPO/plan.md",
  "updated_at": "2026-06-14T12:00:00Z",
  "summary": {
    "total": 2, "green": 2, "red": 0, "pending": 0, "running": 0,
    "stale": 0, "fresh_green": 2
  },
  "next": null,
  "steps": [
    { "id": "s1", "title": "First step", "check": "true", "status": "green",
      "started_at": null, "activity": null,
      "exit_code": 0, "diff_sha": "$sha",
      "last_run": "2026-06-14T12:00:00Z", "evidence_tail": "" },
    { "id": "s2", "title": "Second step", "check": "true", "status": "green",
      "started_at": null, "activity": null,
      "exit_code": 0, "diff_sha": "$sha",
      "last_run": "2026-06-14T12:00:00Z", "evidence_tail": "" }
  ]
}
EOF
}

# AC-8 — a legacy v1 ledger (no new keys) is read + recomputed by `status` without
# error; permissive jq reads the missing keys as null, summary.retried defaults 0.
@test "status: legacy v1 ledger (no new keys) recomputes without error (AC-8)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  local sha
  sha=$(cd "$REPO" && git diff --no-color "main...HEAD" | git hash-object --stdin)
  _write_legacy_ledger "$sha"

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  [[ "$output" == *"next=none"* ]]
  # summary.retried is computed from the missing retries key as 0 (no crash).
  [ "$(jq -r '.summary.retried' "$LEDGER")" = "0" ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "2" ]
}

# AC-8 — after a `run` touches a legacy ledger, every written step entry carries
# the new keys backfilled to defaults (ac_refs:[], depends_on:[], input:null,
# retries:[]). The prior legacy green is NOT archived (only reds are).
@test "run: touching a legacy ledger backfills new keys to defaults (AC-8)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  local sha
  sha=$(cd "$REPO" && git diff --no-color "main...HEAD" | git hash-object --stdin)
  _write_legacy_ledger "$sha"

  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  for id in s1 s2; do
    [ "$(jq -c --arg id "$id" '.steps[] | select(.id==$id) | .ac_refs' "$LEDGER")" = '[]' ]
    [ "$(jq -c --arg id "$id" '.steps[] | select(.id==$id) | .depends_on' "$LEDGER")" = '[]' ]
    [ "$(jq -r --arg id "$id" '.steps[] | select(.id==$id) | .input' "$LEDGER")" = "null" ]
    [ "$(jq -c --arg id "$id" '.steps[] | select(.id==$id) | .retries' "$LEDGER")" = '[]' ]
  done
}

# AC-8 — a legacy plan (steps with only id/title/check) produces a gate-passing
# ledger: a full run of an all-true plan ends 2/2 fresh-green (exit 0, next=none).
@test "run: legacy plan (id/title/check only) produces a gate-passing ledger (AC-8)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"2/2 fresh-green"* ]]
  [[ "$output" == *"next=none"* ]]
  [ "$(jq -r '.next' "$LEDGER")" = "null" ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "2" ]
}

# --- s7: AC coverage in `status` (report-only) ---

# Write a spec doc with AC-1 and AC-2 under the canonical heading.
_write_spec_two() {
  cat > "$1" <<'EOF'
# Some Spec

## Acceptance criteria

- **AC-1:** first criterion.
- **AC-2:** second criterion.
EOF
}

# Write a plan whose **Spec:** points at $3, s1 covers AC-1, s2 covers nothing.
# $1=doc $2=s1 check $3=spec path  $4=s2 check
_write_doc_with_spec() {
  cat > "$1" <<EOF
# Fixture Plan

**Spec:** $3

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "covers AC-1", "check": "$2", "ac_refs": ["AC-1"] },
  { "id": "s2", "title": "no AC ref", "check": "$4" }
]
\`\`\`
EOF
}

# AC-6 — status reports AC-1 covered (fresh-green s1) and AC-2 uncovered.
@test "status: reports covered AC and flags uncovered AC (AC-6)" {
  doc="$REPO/plan.md"; spec="$REPO/spec.md"
  _write_spec_two "$spec"
  _write_doc_with_spec "$doc" "true" "$spec" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  # Report-only: all fresh-green, so the exit code is still 0 despite AC-2 uncovered.
  [ "$status" -eq 0 ]
  [[ "$output" == *"AC coverage"* ]]
  [[ "$output" == *"AC-1: covered by s1"* ]]
  [[ "$output" == *"AC-2: UNCOVERED"* ]]
}

# AC-6 — a covering step that is green-but-STALE does not count as coverage:
# AC-1 is reported UNCOVERED (s1 not fresh-green) even though s1 references it.
@test "status: stale covering step makes its AC uncovered (report-only) (AC-6)" {
  doc="$REPO/plan.md"; spec="$REPO/spec.md"
  _write_spec_two "$spec"
  _write_doc_with_spec "$doc" "true" "$spec" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  # Drift the diff_sha so the green steps go stale.
  ( cd "$REPO" && echo more >> feature.txt && git add feature.txt && git commit -q -m change )

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  # Stale -> not all fresh-green -> exit 1 (driven by staleness, NOT by coverage).
  [ "$status" -eq 1 ]
  [[ "$output" == *"AC-1: UNCOVERED"* ]]
  [[ "$output" == *"not fresh-green"* ]]
}

# AC-10 — with **Spec:** none, status prints NO AC coverage section (report-only,
# no blocker) and the exit code is unaffected.
@test "status: Spec=none skips the AC coverage section entirely (AC-10)" {
  doc="$REPO/plan.md"
  _write_doc_with_spec "$doc" "true" "none" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]

  run bash -c "cd '$REPO' && bash '$SCRIPT' status"
  [ "$status" -eq 0 ]
  [[ "$output" != *"AC coverage"* ]]
}
