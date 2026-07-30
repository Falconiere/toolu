#!/usr/bin/env bats
# Tests for the telemetry write sites instrumented in:
#   - lib/plan-ledger.sh (step_run, on both the full-run and --step paths)
#   - lib/gate-file.sh   (gate_fail / gate_clear, transitions only)
# Real temp git repos, real ledger runs, real gate files. No mocked commands.

bats_require_minimum_version 1.5.0

SCRIPT="${BATS_TEST_DIRNAME}/../plan-ledger.sh"
REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"

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
    git commit -qm base
    git checkout -q -b feat/x
    echo feature > feature.txt
    git add feature.txt
    git commit -qm feature
  )
  export PUSH_REVIEW_BASE=main
  LEDGER="$REPO/.claude/tmp/plan-ledger/feat_x.json"
  TELEMETRY_FILE="$REPO/.claude/tmp/telemetry/feat_x.jsonl"
  GATE="$REPO/.claude/tmp/quality-gate-status.json"
  mkdir -p "$(dirname "$GATE")"

  # Isolated config dirs so telemetry defaults enabled regardless of the host
  # machine's real ~/.claude config (mirrors lib/__tests__/telemetry.bats).
  export HOME="$TMP/home"
  export CLAUDE_PROJECT_DIR="$REPO"
  unset TOOLU_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME TELEMETRY_DIR
  mkdir -p "$HOME/.claude" "$REPO/.claude"

  # shellcheck source=../gate-file.sh
  . "$REPO_ROOT/hooks/lib/gate-file.sh"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Write a 2-step plan doc; checks are passed in so each test controls red/green.
# $1=doc path  $2=s1 check  $3=s2 check
_write_doc() {
  cat > "$1" <<EOF
# Fixture Plan

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "First step", "check": "$2" },
  { "id": "s2", "title": "Second step", "check": "$3" }
]
\`\`\`
EOF
}

# --- lib/plan-ledger.sh: step_run --------------------------------------------

@test "plan-ledger step_run: full run appends one event per step with integer duration_s (AC-3)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s1") | .status' "$TELEMETRY_FILE")" = "green" ]
  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s1") | .exit_code' "$TELEMETRY_FILE")" = "0" ]
  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s1") | .attempt' "$TELEMETRY_FILE")" = "1" ]
  d1=$(jq -r 'select(.event=="step_run" and .step_id=="s1") | .duration_s' "$TELEMETRY_FILE")
  [[ "$d1" =~ ^[0-9]+$ ]]

  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s2") | .status' "$TELEMETRY_FILE")" = "red" ]
  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s2") | .exit_code' "$TELEMETRY_FILE")" = "1" ]
  [ "$(jq -r 'select(.event=="step_run" and .step_id=="s2") | .attempt' "$TELEMETRY_FILE")" = "1" ]
  d2=$(jq -r 'select(.event=="step_run" and .step_id=="s2") | .duration_s' "$TELEMETRY_FILE")
  [[ "$d2" =~ ^[0-9]+$ ]]
}

@test "plan-ledger step_run: --step run appends only the targeted step; attempt increments on retry (AC-3)" {
  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "false"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 1 ]
  # Isolate the --step run's own telemetry from the full run's.
  rm -f "$TELEMETRY_FILE"

  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc' --step s2"
  [ "$status" -eq 0 ]

  [ -f "$TELEMETRY_FILE" ]
  [ "$(wc -l < "$TELEMETRY_FILE" | tr -d ' ')" = "1" ]
  [ "$(jq -r '.step_id' "$TELEMETRY_FILE")" = "s2" ]
  [ "$(jq -r '.status' "$TELEMETRY_FILE")" = "green" ]
  # The prior red is archived into retries[] before this event fires -> attempt 2.
  [ "$(jq -r '.attempt' "$TELEMETRY_FILE")" = "2" ]
}

@test "plan-ledger step_run: telemetry.enabled=false -> run still succeeds, no telemetry file (AC-4)" {
  echo '{"version":1,"telemetry":{"enabled":false}}' > "$REPO/.claude/toolu.config.json"

  doc="$REPO/plan.md"
  _write_doc "$doc" "true" "true"
  run bash -c "cd '$REPO' && bash '$SCRIPT' run '$doc'"
  [ "$status" -eq 0 ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
  [ -f "$LEDGER" ]
  [ "$(jq -r '.summary.fresh_green' "$LEDGER")" = "2" ]
}

# --- lib/gate-file.sh: gate_fail / gate_clear (transitions only) ------------

@test "gate-file: gate_record_failure appends a gate_fail event (AC-4)" {
  gate_record_failure "$GATE" "/p/a.ts" "ts-quality-hook" "bad" "viol-a\n"

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "gate_fail" ]
  [ "$(jq -r '.file' "$TELEMETRY_FILE")" = "/p/a.ts" ]
  [ "$(jq -r '.source' "$TELEMETRY_FILE")" = "ts-quality-hook" ]
}

@test "gate-file: gate_clear_file on an owned entry appends a gate_clear event (AC-4)" {
  gate_record_failure "$GATE" "/p/a.ts" "ts-quality-hook" "bad" "viol-a\n"
  rm -f "$TELEMETRY_FILE"

  gate_clear_file "$GATE" "/p/a.ts" "ts-quality-hook"

  [ -f "$TELEMETRY_FILE" ]
  [ "$(jq -r '.event' "$TELEMETRY_FILE")" = "gate_clear" ]
  [ "$(jq -r '.file' "$TELEMETRY_FILE")" = "/p/a.ts" ]
  [ "$(jq -r '.source' "$TELEMETRY_FILE")" = "ts-quality-hook" ]
}

@test "gate-file: gate_clear_file early return (other source owns the entry) appends nothing (AC-4)" {
  gate_record_failure "$GATE" "/p/a.ts" "ts-quality-hook" "bad" "viol-a\n"
  rm -f "$TELEMETRY_FILE"

  gate_clear_file "$GATE" "/p/a.ts" "rust-quality-hook"

  # Gate unaffected (existing gate-file.bats already asserts this); the point
  # here is that the no-op early return must not write telemetry either.
  [ "$(jq -r '.source' "$GATE")" = "ts-quality-hook" ]
  [ ! -e "$TELEMETRY_FILE" ]
}

@test "gate-file: gate_clear_file on a missing gate file (early return) appends nothing (AC-4)" {
  rm -f "$GATE"
  gate_clear_file "$GATE" "/p/a.ts" "ts-quality-hook"
  [ ! -e "$TELEMETRY_FILE" ]
}

@test "gate-file: telemetry.enabled=false -> gate_record_failure/gate_clear_file still succeed, no telemetry file (AC-4)" {
  echo '{"version":1,"telemetry":{"enabled":false}}' > "$REPO/.claude/toolu.config.json"

  gate_record_failure "$GATE" "/p/a.ts" "ts-quality-hook" "bad" "viol-a\n"
  [ "$(jq -r '.status' "$GATE")" = "failing" ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]

  gate_clear_file "$GATE" "/p/a.ts" "ts-quality-hook"
  [ "$(jq -r '.status' "$GATE")" = "passing" ]
  [ ! -d "$REPO/.claude/tmp/telemetry" ]
}
