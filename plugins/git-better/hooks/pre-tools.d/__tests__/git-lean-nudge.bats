#!/usr/bin/env bats
# git-lean-nudge — Pillar-1 fire/silence + pipe-silence + fail-soft. AC#10,11,16.

SCRIPT="${BATS_TEST_DIRNAME}/../git-lean-nudge.sh"
TOOLU_LIB_DIR="$(cd "${BATS_TEST_DIRNAME}/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

run_cmd() {  # $1 = shell command string
  local input
  input="$(jq -n --arg c "$1" '{tool_input:{command:$c}}')"
  tool_name="Bash" input="$input" bash "$SCRIPT"
}

@test "fires on bare git diff (AC#10)" {
  run run_cmd "git diff HEAD"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb diff")'
}

@test "fires on bare git log (AC#10)" {
  run run_cmd "git log"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb log")'
}

@test "fires on bare git status" {
  run run_cmd "git status"
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb status")'
}

@test "silent on git diff --stat (lean flag present) (AC#10)" {
  run run_cmd "git diff --stat"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "silent on git status -sb (AC#10)" {
  run run_cmd "git status -sb"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "silent on git log --oneline -n 20" {
  run run_cmd "git log --oneline -n 20"
  [ -z "$output" ]
}

@test "silent on write verb git commit (Pillar-1 ignores) (AC#10)" {
  run run_cmd 'git commit -m "x"'
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "silent on git push (AC#10)" {
  run run_cmd "git push origin HEAD"
  [ -z "$output" ]
}

@test "silent when diff piped to head (AC#16)" {
  run run_cmd "git diff | head -50"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "silent when log piped to wc (AC#16)" {
  run run_cmd "git log | wc -l"
  [ -z "$output" ]
}

@test "silent on git diff <path> (targeted)" {
  run run_cmd "git diff src/auth.ts"
  [ -z "$output" ]
}

@test "silent on git diff <flat filename> (targeted, no slash)" {
  run run_cmd "git diff README.md"
  [ -z "$output" ]
}

@test "still fires on git diff HEAD (untargeted ref)" {
  run run_cmd "git diff HEAD"
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb diff")'
}

@test "fail-soft: no TOOLU_LIB_DIR → exit 0, silent (AC#11)" {
  local input
  input="$(jq -n '{tool_input:{command:"git diff"}}')"
  run env -u TOOLU_LIB_DIR tool_name="Bash" input="$input" bash "$SCRIPT"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "no-op on unrelated tool" {
  run env tool_name="Read" input='{}' bash "$SCRIPT"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
