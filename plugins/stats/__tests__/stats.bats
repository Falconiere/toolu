#!/usr/bin/env bats
# stats.sh — end-to-end. Fully isolated under a temp CLAUDE_CONFIG_DIR with a
# built projects tree; no live data is read and the real cache is untouched.

setup() {
  export TZ=UTC
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  SCRIPT="${BATS_TEST_DIRNAME}/../scripts/stats.sh"
  FX="${BATS_TEST_DIRNAME}/fixtures"
  SLUG="-Volumes-Projects-toolu-sh"
  ROOT="$CLAUDE_CONFIG_DIR/projects/$SLUG"
  mkdir -p "$ROOT"
  cp "$FX/multimodel.jsonl" "$ROOT/sess-mm.jsonl"
  cp "$FX/sub-session.jsonl" "$ROOT/sess-sub.jsonl"
  mkdir -p "$ROOT/sess-sub/subagents"
  cp "$FX"/sub-session/subagents/agent-*.jsonl "$ROOT/sess-sub/subagents/"
}

run_stats() { run bash "$SCRIPT" "$@"; }

@test "default digest renders the dashboard from real transcripts" {
  run_stats
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Claude Code Usage"   # boxed header
  echo "$output" | grep -q "┌"                    # box rule
  echo "$output" | grep -q "█"                     # a bar/gauge cell from real data
  echo "$output" | grep -q "Trend 14d"             # sparkline section
  echo "$output" | grep -q "toolu.sh"              # real project (multimodel .cwd)
  echo "$output" | grep -q "Activity"
}

@test "--json emits a valid aggregate with both sessions" {
  run_stats --json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.totals.sessions == 2' >/dev/null
  echo "$output" | jq -e '.by_model|length >= 1' >/dev/null
}

@test "--html writes a self-contained report from real transcripts (no browser)" {
  run env STATS_NO_OPEN=1 bash "$SCRIPT" --html
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Wrote HTML report"
  local f="$CLAUDE_CONFIG_DIR/stats/report.html"
  [ -f "$f" ]
  grep -q "<!DOCTYPE html>" "$f"
  grep -q "Claude Code Usage" "$f"
  grep -qF "toolu.sh" "$f"                          # real project populated a row
  grep -qF 'class="fill" style="width:' "$f"        # a CSS bar from real data
  grep -qF '<svg class="spark"' "$f"                # sparkline rendered
  ! grep -qF "{{" "$f"                              # no placeholder left
}

@test "--today records the today window" {
  run_stats --json --today
  [ "$(echo "$output" | jq -r '.window')" = "today" ]
}

@test "--session restricts to one session" {
  run_stats --json --session sess-mm
  [ "$(echo "$output" | jq -r '.totals.sessions')" = "1" ]
  [ "$(echo "$output" | jq -r '.top_sessions[0].session_id')" = "sess-mm" ]
}

@test "--project restricts to that project" {
  # only multimodel carries .cwd → project toolu.sh; sub-session (no cwd) falls
  # back to its slug, so the toolu.sh filter selects exactly one session.
  run_stats --json --project toolu.sh
  [ "$(echo "$output" | jq -r '.totals.sessions')" = "1" ]
  [ "$(echo "$output" | jq -r '.by_project[0].project')" = "toolu.sh" ]
}

@test "--model narrows the report and nulls windows" {
  run_stats --json --model haiku
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.windows')" = "null" ]
  echo "$output" | jq -e '[.by_model[].model] | all(test("haiku"))' >/dev/null
}

@test "--rescan recomputes and writes the cache" {
  run_stats --rescan --json
  [ "$status" -eq 0 ]
  [ -f "$CLAUDE_CONFIG_DIR/stats/sessions/sess-mm.json" ]
}

@test "--this-session reports the newest transcript under the cwd" {
  work="$BATS_TEST_TMPDIR/proj"
  mkdir -p "$work"
  slug="$(printf '%s' "$work" | sed 's/[^A-Za-z0-9]/-/g')"
  mkdir -p "$CLAUDE_CONFIG_DIR/projects/$slug"
  cp "$FX/multimodel.jsonl" "$CLAUDE_CONFIG_DIR/projects/$slug/live.jsonl"
  run bash -c "cd '$work' && CLAUDE_CONFIG_DIR='$CLAUDE_CONFIG_DIR' TZ=UTC bash '$SCRIPT' --this-session --json"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.totals.sessions')" = "1" ]
  [ "$(echo "$output" | jq -r '.top_sessions[0].session_id')" = "live" ]
}

@test "--since drops sessions with no activity on/after the date" {
  run_stats --json --since 2099-01-01
  [ "$(echo "$output" | jq -r '.totals.sessions')" = "0" ]
}

@test "unknown option exits non-zero with usage" {
  run_stats --bogus
  [ "$status" -eq 2 ]
}

@test "missing jq prints a notice and exits 0" {
  BASH_BIN="$(command -v bash)"
  run env PATH="" "$BASH_BIN" "$SCRIPT"   # empty PATH → jq unfindable
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "jq not found"
}

@test "empty config dir reports no usage, not an error" {
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/empty"
  mkdir -p "$CLAUDE_CONFIG_DIR/projects"
  run_stats
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no usage recorded"
}

@test "--json always carries the API billing lens; no plan -> subscription null" {
  run_stats --json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.billing.api.cost >= 0' >/dev/null
  [ "$(echo "$output" | jq -r '.billing.subscription')" = "null" ]
}

@test "--plan adds the subscription lens with the right fee" {
  run_stats --json --plan max5
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.billing.plan')" = "max5" ]
  [ "$(echo "$output" | jq -r '.billing.plan_fee_monthly')" = "100" ]
  echo "$output" | jq -e '.billing.subscription != null' >/dev/null
}

@test "--billing sets the lens mode carried in the aggregate" {
  run_stats --json --billing subscription
  [ "$(echo "$output" | jq -r '.billing.mode')" = "subscription" ]
}

@test "stats.conf supplies the plan when no --plan flag is given" {
  printf 'plan=pro\n' > "$CLAUDE_CONFIG_DIR/stats.conf"
  run_stats --json
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.billing.plan')" = "pro" ]
  [ "$(echo "$output" | jq -r '.billing.plan_fee_monthly')" = "20" ]
}
