#!/usr/bin/env bats
# live-headless.bats — HERMETIC tests for the live cavecrew + whole-session
# runners. NO `claude` CLI is ever invoked and there is no network: we prove the
# measurement BACKEND (stats_usage_rollup over a real transcript fixture set) and
# the runner CONTRACT (arg-validation, schema-valid output, executable bits)
# without paying for a live A/B. The live runs themselves are manual.

setup() {
  BENCH_DIR="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"                 # benchmarks/
  CAVECREW="$BENCH_DIR/cases/cavecrew/run.sh"
  WHOLE="$BENCH_DIR/cases/whole-session/run.sh"
  FIX="$BENCH_DIR/__tests__/fixtures"
  export BENCH_RESULTS_DIR="$BATS_TEST_TMPDIR/results"
}

# --- WIRING: the cavecrew/whole-session measurement backend ------------------
# Feed the REAL sub-session transcript + its subagent transcripts through
# stats_usage_rollup (the exact call both runners make) and prove it yields a
# usable rollup. This is the key test: it proves the backend is wired correctly.
@test "stats_usage_rollup rolls up the real sub-session transcript set" {
  run bash -c '
    set -e
    # shellcheck source=/dev/null
    source "$1/lib/common.sh"
    roll="$(stats_usage_rollup "$2/sub-session.jsonl" "$2"/sub-session/subagents/agent-*.jsonl)"
    printf "%s" "$roll" | jq -e "type == \"object\"" >/dev/null
    printf "%s" "$roll" | jq -e ".messages > 0" >/dev/null
    printf "%s" "$roll" | jq -e "(.totals.tokens | type) == \"number\"" >/dev/null
    printf "%s\n" "$roll" | jq -r ".messages"
  ' _ "$BENCH_DIR" "$FIX"
  [ "$status" -eq 0 ]
  # last line is the message count; must be a positive integer
  [ "${lines[-1]}" -gt 0 ]
}

# --- ARG VALIDATION: live tier needs the claude CLI -------------------------
# Strip /opt/homebrew/bin (where `claude` lives) from PATH but keep bash/jq/git.
# Both runners must refuse with a non-zero status and a message naming the CLI.
@test "cavecrew/run.sh fails clearly when the claude CLI is absent" {
  PATH="/usr/bin:/bin" run bash "$CAVECREW"
  [ "$status" -ne 0 ]
  [[ "$output" == *"claude CLI"* ]]
}

@test "whole-session/run.sh fails clearly when the claude CLI is absent" {
  PATH="/usr/bin:/bin" run bash "$WHOLE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"claude CLI"* ]]
}

@test "cavecrew/run.sh rejects an unknown arg with status 2" {
  run bash "$CAVECREW" --bogus
  [ "$status" -eq 2 ]
}

@test "whole-session/run.sh rejects an unknown arg with status 2" {
  run bash "$WHOLE" --bogus
  [ "$status" -eq 2 ]
}

# --- EXECUTABLE BITS --------------------------------------------------------
@test "both live run.sh are executable" {
  [ -x "$CAVECREW" ]
  [ -x "$WHOLE" ]
}

# --- SCHEMA: hand-built samples pass bench_result_validate ------------------
@test "a hand-built cavecrew result passes bench_result_validate" {
  out="$BENCH_RESULTS_DIR/cavecrew-live-2026-06-15.json"
  mkdir -p "$BENCH_RESULTS_DIR"
  jq -n '{
    mechanism:"cavecrew", tier:"live", method:"headless-ab",
    tokenizer:{mode:"usage", source:"usage"},
    provenance:{model:"claude-sonnet-4-6", date:"2026-06-15", commit:"deadbeef", n_runs:5, pricing_id:"2026-06"},
    baseline: {label:"inline",   tokens:{input:null,output:null,cache_read:null,cache_write:null,total:90000}, cost:null},
    treatment:{label:"cavecrew", tokens:{input:null,output:null,cache_read:null,cache_write:null,total:40000}, cost:null},
    delta:{tokens_pct:55, cost_pct:null, abs_tokens:50000, mean:40000, stddev:1200},
    cases:[{id:"list-bats-tests", baseline_tokens:90000, treatment_tokens:40000}],
    notes:"hermetic sample"
  }' > "$out"
  run bash -c 'source "$1/lib/result.sh"; bench_result_validate "$2"' _ "$BENCH_DIR" "$out"
  [ "$status" -eq 0 ]
}

# --- EMIT PIPELINE: stub the `claude` boundary, run the REAL pipeline ---------
# A stub `claude` writes the REAL sub-session transcript fixture at the pinned
# session path and prints a real --output-format json result. Everything else —
# transcript resolution, stats_usage_rollup, bench_stats, the jq emit, and
# bench_result_write + validation — is the real code under test.
_make_claude_stub() {  # $1 = bin dir
  mkdir -p "$1"
  cat > "$1/claude" <<STUB
#!/usr/bin/env bash
sid=""
while [ \$# -gt 0 ]; do case "\$1" in --session-id) sid="\$2"; shift 2;; *) shift;; esac; done
root="\${CLAUDE_CONFIG_DIR:-\$HOME/.claude}/projects"
slug="\$(printf '%s' "\$PWD" | sed 's/[^A-Za-z0-9]/-/g')"
dir="\$root/\$slug"
mkdir -p "\$dir/\$sid/subagents"
cp "$FIX/sub-session.jsonl" "\$dir/\$sid.jsonl"
cp "$FIX"/sub-session/subagents/agent-*.jsonl "\$dir/\$sid/subagents/" 2>/dev/null || true
printf '%s' '{"total_cost_usd":0.0123,"result":"ok"}'
STUB
  chmod +x "$1/claude"
}

@test "stubbed cavecrew run drives the real emit pipeline to a schema-valid result" {
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  _make_claude_stub "$BATS_TEST_TMPDIR/bin"
  PATH="$BATS_TEST_TMPDIR/bin:$PATH" run bash "$CAVECREW" --n 1
  [ "$status" -eq 0 ]
  local out; out="$(ls "$BENCH_RESULTS_DIR"/cavecrew-live-*.json | head -1)"
  [ -f "$out" ]
  [ "$(jq -r '.mechanism'         "$out")" = "cavecrew" ]
  [ "$(jq -r '.tokenizer.mode'    "$out")" = "usage" ]
  [ "$(jq -r '.delta.tokens_pct'  "$out")" != "null" ]
  run bash -c 'source "$1/lib/result.sh"; bench_result_validate "$2"' _ "$BENCH_DIR" "$out"
  [ "$status" -eq 0 ]
}

@test "stubbed whole-session run drives the real emit pipeline (incl cost) to a schema-valid result" {
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  _make_claude_stub "$BATS_TEST_TMPDIR/bin"
  PATH="$BATS_TEST_TMPDIR/bin:$PATH" run bash "$WHOLE" --n 1
  [ "$status" -eq 0 ]
  local out; out="$(ls "$BENCH_RESULTS_DIR"/whole-session-live-*.json | head -1)"
  [ -f "$out" ]
  [ "$(jq -r '.mechanism'         "$out")" = "whole-session" ]
  [ "$(jq -r '.baseline.cost'     "$out")" != "null" ]   # cost path exercised
  [ "$(jq -r '.delta.cost_pct'    "$out")" != "null" ]
  run bash -c 'source "$1/lib/result.sh"; bench_result_validate "$2"' _ "$BENCH_DIR" "$out"
  [ "$status" -eq 0 ]
}

@test "a hand-built whole-session result passes bench_result_validate" {
  out="$BENCH_RESULTS_DIR/whole-session-live-2026-06-15.json"
  mkdir -p "$BENCH_RESULTS_DIR"
  jq -n '{
    mechanism:"whole-session", tier:"live", method:"headless-ab",
    tokenizer:{mode:"usage", source:"usage"},
    provenance:{model:"claude-sonnet-4-6", date:"2026-06-15", commit:"deadbeef", n_runs:5, pricing_id:"2026-06"},
    baseline: {label:"toolu-off", tokens:{input:null,output:null,cache_read:null,cache_write:null,total:120000}, cost:0.42},
    treatment:{label:"toolu-on",  tokens:{input:null,output:null,cache_read:null,cache_write:null,total:95000}, cost:0.31},
    delta:{tokens_pct:20, cost_pct:26, abs_tokens:25000, mean:0.31, stddev:0.02},
    cases:[{id:"summarize-bench-lib", baseline_tokens:120000, treatment_tokens:95000}],
    notes:"AGGREGATE whole-session delta; NOT the sum of per-mechanism deltas."
  }' > "$out"
  run bash -c 'source "$1/lib/result.sh"; bench_result_validate "$2"' _ "$BENCH_DIR" "$out"
  [ "$status" -eq 0 ]
}
