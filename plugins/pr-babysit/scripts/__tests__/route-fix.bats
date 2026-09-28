#!/usr/bin/env bats
# Real-data tests for scripts/route-fix.sh (AC-1, AC-2, AC-3).
#
# Inputs: fixtures/items/review-items.json — six real CI-reviewer threads from
# the captured snapshots (Falconiere/comemory#216, Falconiere/toolu#115, #165)
# with controller-written tasks; fixtures/jev/fix-tiers.json — the answer map
# a live `jev.sh ask` (jev-1.13.0) returned for exactly route-fix.sh's
# question on those items, captured 2026-09-28; real toolu.config.json files.
#
# The host-CLI probe is an environment observation, so every test runs under
# a controlled PATH: $TMP/bin (jq plus the host CLIs that should be "present"
# as real executables) followed by /usr/bin:/bin, which hold no agent CLI.
# The only stub is PB_JEV pointing at a script that exits 1 — a controlled
# external failure proving the fallback. No mocks.

SCRIPTS="${BATS_TEST_DIRNAME}/.."
FX="${BATS_TEST_DIRNAME}/fixtures"
ITEMS="$FX/items/review-items.json"
ANSWERS="$FX/jev/fix-tiers.json"
NOW=2026-09-28T12:00:00Z
SEC=PRRT_kwDOSzYYFc6jy6Au    # medium SECURITY: count_table table name (task: SQL injection)
ORM=PRRT_kwDOSzYYFc6jy5_u    # high PERFORMANCE: prepare_cached per call
CIR=PRRT_kwDOSzUwAc6K6nEk    # high CORRECTNESS: CI_REVIEWER login form
RDM=PRRT_kwDOSzUwAc6d2Ypf    # medium DOC/COMMENT: README version
SCH=PRRT_kwDOSzYYFc6jy6BF    # nit DOC/COMMENT: schema_core doc comment
AGT=PRRT_kwDOSzUwAc6K6VND    # low DOC/COMMENT: AGENTS.md removal note

setup() {
  TMP=$(mktemp -d)
  mkdir -p "$TMP/bin" "$TMP/user" "$TMP/project/.claude"
  ln -s "$(command -v jq)" "$TMP/bin/jq"
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  # Kept for the one live test; every other test runs without a key.
  LIVE_KEY="${TYPESAFE_API_KEY:-}"
  LIVE_JEV="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/jev/jev.sh"
  unset TYPESAFE_API_KEY PB_JEV
  export CLAUDE_CONFIG_DIR="$TMP/no-claude-home" CODEX_HOME="$TMP/no-codex-home"
  clis claude codex cursor-agent
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# clis NAME... -> exactly these host CLIs are on the controlled PATH
clis() {
  rm -f "$TMP/bin/claude" "$TMP/bin/codex" "$TMP/bin/cursor-agent"
  local c
  for c in "$@"; do printf '#!/bin/sh\nexit 0\n' >"$TMP/bin/$c"; chmod +x "$TMP/bin/$c"; done
}

route() {
  PATH="$TMP/bin:/usr/bin:/bin" bash "$SCRIPTS/route-fix.sh" --items "$ITEMS" --now "$NOW" "$@"
}

project_config() { jq -n --argjson v "$1" '{prBabysit:$v}' >"$TMP/project/.claude/toolu.config.json"; }

@test "AC-1: heuristic tiers group high->low on the controller host with epic's Claude table" {
  out=$(route --host claude --no-jev)
  [ "$(jq -r '.source' <<<"$out")" = heuristic ]
  [ "$(jq -r '.dispatch' <<<"$out")" = herdr ]
  [ "$(jq -r '.unattended' <<<"$out")" = true ]
  [ "$(jq -c '[.groups[] | .tier]' <<<"$out")" = '["critical","complex","standard","trivial"]' ]
  [ "$(jq -c '[.groups[] | .seq]' <<<"$out")" = '[1,2,3,4]' ]
  [ "$(jq -c --arg s "$SEC" '.groups[0].items' <<<"$out")" = "[\"$SEC\"]" ]
  [ "$(jq -c '.groups[1].items | sort' <<<"$out")" = "$(jq -nc --arg a "$ORM" --arg b "$CIR" '[$a,$b] | sort')" ]
  [ "$(jq -c '.groups[2].items | sort' <<<"$out")" = "$(jq -nc --arg a "$RDM" --arg b "$AGT" '[$a,$b] | sort')" ]
  [ "$(jq -c '.groups[3].items' <<<"$out")" = "[\"$SCH\"]" ]
  [ "$(jq -c '[.groups[] | [.host, .model, .effort, .class]]' <<<"$out")" = '[["claude","opus","xhigh","architecture"],["claude","opus","high","architecture"],["claude","sonnet","medium","implementation"],["claude","sonnet","low","mechanical"]]' ]
  [ "$(jq -r '.note' <<<"$out")" = "jev disabled; heuristic tiers used" ]
}

@test "AC-1 boundary: an empty items array and a task-less item are plan_invalid (exit 3)" {
  echo '{"round":1,"items":[]}' >"$TMP/empty.json"
  run bash -c "PATH='$TMP/bin:/usr/bin:/bin' bash '$SCRIPTS/route-fix.sh' --items '$TMP/empty.json' --host claude --no-jev"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = plan_invalid ]
  jq '.items[0].task = ""' "$ITEMS" >"$TMP/notask.json"
  run bash -c "PATH='$TMP/bin:/usr/bin:/bin' bash '$SCRIPTS/route-fix.sh' --items '$TMP/notask.json' --host claude --no-jev"
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].message' <<<"$output")" = "every item needs a task" ]
}

@test "AC-2: pool + prefer + routing override: critical on cursor, the rest on codex with the override" {
  project_config '{"hosts":["codex","cursor"],"prefer":{"critical":["cursor"]},"unattended":false,
    "routing":{"codex":[{"model":"gpt-6-luna","effort":"low"},{"model":"gpt-6-luna","effort":"medium"},{"model":"gpt-6-sol","effort":"high"},{"model":"gpt-6-sol","effort":"xhigh"}]}}'
  out=$(route --host claude --no-jev)
  [ "$(jq -c '.groups[0] | [.tier, .host, .model]' <<<"$out")" = '["critical","cursor","gpt-5.6-sol-xhigh"]' ]
  [ "$(jq -c '[.groups[1:][] | [.host, .model, .effort]]' <<<"$out")" = '[["codex","gpt-6-sol","high"],["codex","gpt-6-luna","medium"],["codex","gpt-6-luna","low"]]' ]
  [ "$(jq -r '.unattended' <<<"$out")" = false ]
}

@test "AC-2: an unexpired codex cooldown moves every group to cursor with Cursor's table model" {
  project_config '{"hosts":["codex","cursor"],"prefer":{"critical":["cursor"]}}'
  jq -n '{version:2, hostCooldowns:{codex:{until:"2026-09-28T12:30:00Z", reason:"host_limited"}}}' >"$TMP/state.json"
  out=$(route --host claude --no-jev --state-file "$TMP/state.json")
  [ "$(jq -c '[.groups[] | [.tier, .host, .model]]' <<<"$out")" = '[["critical","cursor","gpt-5.6-sol-xhigh"],["complex","cursor","claude-opus-5-thinking-high"],["standard","cursor","gpt-5.6-sol-high"],["trivial","cursor","composer-2.5"]]' ]
  jq -r '.note' <<<"$out" | grep -q 'cooling: codex'
}

@test "AC-2 boundary: an expired cooldown is ignored" {
  project_config '{"hosts":["codex","cursor"]}'
  jq -n '{version:2, hostCooldowns:{codex:{until:"2026-09-28T11:59:59Z", reason:"host_limited"}}}' >"$TMP/state.json"
  out=$(route --host claude --no-jev --state-file "$TMP/state.json")
  [ "$(jq -c '[.groups[] | .host] | unique' <<<"$out")" = '["codex"]' ]
}

@test "AC-2 boundary: every pool host cooling -> inline, null hosts, a note naming them" {
  project_config '{"hosts":["codex","cursor"]}'
  jq -n '{version:2, hostCooldowns:{codex:{until:"2026-09-28T13:00:00Z"}, cursor:{until:"2026-09-28T13:00:00Z"}}}' >"$TMP/state.json"
  out=$(route --host claude --no-jev --state-file "$TMP/state.json")
  [ "$(jq -r '.dispatch' <<<"$out")" = inline ]
  [ "$(jq -c '[.groups[] | [.host, .model, .effort]] | unique' <<<"$out")" = '[[null,null,null]]' ]
  [ "$(jq -c '[.groups[] | .class]' <<<"$out")" = '["architecture","architecture","implementation","mechanical"]' ]
  jq -r '.note' <<<"$out" | grep -q 'no fixer host available (cooling: codex, cursor)'
}

@test "AC-2 boundary: a pool host whose CLI is not on PATH is dropped with a note" {
  project_config '{"hosts":["cursor","codex"]}'
  clis claude codex
  out=$(route --host claude --no-jev)
  [ "$(jq -c '[.groups[] | .host] | unique' <<<"$out")" = '["codex"]' ]
  jq -r '.note' <<<"$out" | grep -q 'dropped (CLI not on PATH): cursor'
}

@test "AC-2 boundary: an unknown pool host is config_invalid (exit 3)" {
  project_config '{"hosts":["gemini"]}'
  run route --host claude --no-jev
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = config_invalid ]
}

@test "AC-3: a replayed real Jev answer maps score -> tier with round(score+0.15) and keeps confidence" {
  out=$(route --host claude --jev-answers-in "$ANSWERS")
  [ "$(jq -r '.source' <<<"$out")" = jev ]
  # Captured scores: SEC 1.92 -> 2, ORM 1.9 -> 2, the four doc/login items <= 0.11 -> 0.
  [ "$(jq -c '[.groups[] | [.tier, (.items | length)]]' <<<"$out")" = '[["complex",2],["trivial",4]]' ]
  [ "$(jq -c '.groups[0].items | sort' <<<"$out")" = "$(jq -nc --arg a "$SEC" --arg b "$ORM" '[$a,$b] | sort')" ]
  [ "$(jq -r --arg s "$SEC" '.items[] | select(.id == $s) | [.score, .confidence, .tier] | @csv' <<<"$out")" = '1.92,0.08,"complex"' ]
  [ "$(jq -r '.note' <<<"$out")" = null ]
}

@test "AC-3: --raise lifts exactly that item one tier; critical stays critical" {
  out=$(route --host claude --jev-answers-in "$ANSWERS" --raise "$RDM")
  [ "$(jq -r --arg r "$RDM" '.items[] | select(.id == $r) | [.tier, .raised] | @csv' <<<"$out")" = '"standard",true' ]
  [ "$(jq -r --arg a "$AGT" '.items[] | select(.id == $a) | [.tier, .raised] | @csv' <<<"$out")" = '"trivial",false' ]
  out=$(route --host claude --no-jev --raise "$SEC")
  [ "$(jq -r --arg s "$SEC" '.items[] | select(.id == $s) | .tier' <<<"$out")" = critical ]
  run route --host claude --no-jev --raise PRRT_not_in_items
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].code' <<<"$output")" = plan_invalid ]
}

@test "AC-3 boundary: an answer map missing one item -> source mixed, heuristic tier for that item" {
  jq --arg s "$SEC" 'del(.[$s])' "$ANSWERS" >"$TMP/partial.json"
  out=$(route --host claude --jev-answers-in "$TMP/partial.json")
  [ "$(jq -r '.source' <<<"$out")" = mixed ]
  [ "$(jq -r --arg s "$SEC" '.items[] | select(.id == $s) | [.source, .tier] | @csv' <<<"$out")" = '"heuristic","critical"' ]
}

@test "AC-3 boundary: a Jev that exits non-zero falls back to the heuristic with a note naming the exit" {
  printf '#!/bin/sh\necho "service down" >&2\nexit 1\n' >"$TMP/jev-down.sh"
  out=$(PB_JEV="$TMP/jev-down.sh" TYPESAFE_API_KEY=unused route --host claude)
  [ "$(jq -r '.source' <<<"$out")" = heuristic ]
  jq -r '.note' <<<"$out" | grep -q 'jev failed (exit 1): service down'
}

@test "AC-3 boundary: no jev.sh installed or no key -> heuristic with the reason" {
  out=$(route --host claude)
  jq -r '.note' <<<"$out" | grep -q 'jev.sh not installed'
  printf '#!/bin/sh\nexit 0\n' >"$TMP/jev-ok.sh"
  out=$(PB_JEV="$TMP/jev-ok.sh" route --host claude)
  jq -r '.note' <<<"$out" | grep -q 'TYPESAFE_API_KEY not set'
}

@test "AC-3 live: a real Jev call tiers every item (runs when a key and jev.sh exist)" {
  [ -n "$LIVE_KEY" ] && [ -f "$LIVE_JEV" ] || skip "no TYPESAFE_API_KEY / jev.sh on this machine"
  out=$(PB_JEV="$LIVE_JEV" TYPESAFE_API_KEY="$LIVE_KEY" route --host claude)
  # A service outage is the documented fallback (covered above), not a regression here.
  if jq -r '.note // ""' <<<"$out" | grep -q '^jev failed'; then skip "$(jq -r '.note' <<<"$out")"; fi
  [ "$(jq -r '.source' <<<"$out")" = jev ]
  [ "$(jq '[.items[] | select(.score != null)] | length' <<<"$out")" -eq 6 ]
}
