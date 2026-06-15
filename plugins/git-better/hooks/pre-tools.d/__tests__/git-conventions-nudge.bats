#!/usr/bin/env bats
# git-conventions-nudge — Pillar-2 fire on commit/pr-create, cached summary,
# fail-soft. AC#10 (Pillar 2), #11.

SCRIPT="${BATS_TEST_DIRNAME}/../git-conventions-nudge.sh"
TOOLU_LIB_DIR="$(cd "${BATS_TEST_DIRNAME}/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

run_cmd() {  # $1 = shell command string
  local input
  input="$(jq -n --arg c "$1" '{tool_input:{command:$c}}')"
  tool_name="Bash" input="$input" bash "$SCRIPT"
}

@test "fires on git commit (AC#10 Pillar2)" {
  run run_cmd 'git commit -m "feat: x"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb conventions")'
}

@test "fires on git commit --amend" {
  run run_cmd 'git commit --amend --no-edit'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb conventions")'
}

@test "fires on gh pr create (AC#10 Pillar2)" {
  run run_cmd 'gh pr create --fill'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("gb conventions")'
}

@test "silent on git status" {
  run run_cmd "git status"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "silent on git diff" {
  run run_cmd "git diff"
  [ -z "$output" ]
}

@test "includes cached repo summary when profile exists" {
  export TOOLU_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q
  git config user.email t@example.com; git config user.name tester
  printf 'a\n' > src.ts; git add -A; git commit -qm "feat(core): init (#1)"
  # Populate the cache with a real profile.
  bash "$BATS_TEST_DIRNAME/../../../scripts/git-better.sh" conventions --json >/dev/null
  run run_cmd 'git commit -m "feat: y"'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("commit conventional-commits")'
}

@test "fail-soft: no TOOLU_LIB_DIR → exit 0, silent (AC#11)" {
  local input
  input="$(jq -n '{tool_input:{command:"git commit -m x"}}')"
  run env -u TOOLU_LIB_DIR tool_name="Bash" input="$input" bash "$SCRIPT"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
