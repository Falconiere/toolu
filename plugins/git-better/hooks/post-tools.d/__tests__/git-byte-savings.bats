#!/usr/bin/env bats
# git-byte-savings — records gb vs git-raw output bytes to the shared ledger.
# AC#12: real returned bytes, no fabricated counterfactual.

SCRIPT="${BATS_TEST_DIRNAME}/../git-byte-savings.sh"

setup() {
  export TOOLU_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  LEDGER="$TOOLU_CONFIG_DIR/toolu/byte-savings/s1.jsonl"
}

run_post() {  # $1 = command, $2 = response text
  local input
  input="$(jq -n --arg c "$1" --arg r "$2" '{tool_input:{command:$c},tool_response:$r,session_id:"s1"}')"
  tool_name="Bash" input="$input" bash "$SCRIPT"
}

@test "gb call recorded as kind gb with real byte count" {
  run_post "gb diff" "abcde"   # 5 bytes
  [ -f "$LEDGER" ]
  run jq -e 'select(.kind=="gb") | .returned == 5 and .full == 0' "$LEDGER"
  [ "$status" -eq 0 ]
}

@test "shim-path gb call also classified as gb" {
  run_post "/home/u/.claude/toolu/bin/gb status" " M src.ts"
  grep -q '"kind":"gb"' "$LEDGER"
}

@test "bare raw git read recorded as git-raw" {
  run_post "git diff HEAD" "lots of hunks here"
  grep -q '"kind":"git-raw"' "$LEDGER"
}

@test "no counterfactual: full is always 0" {
  run_post "git log" "abc def"
  run jq -e '.full == 0' "$LEDGER"
  [ "$status" -eq 0 ]
}

@test "non-git command writes no record" {
  run_post "ls -la" "a b c"
  [ ! -f "$LEDGER" ]
}

@test "empty response writes no record" {
  run_post "gb diff" ""
  [ ! -f "$LEDGER" ]
}
