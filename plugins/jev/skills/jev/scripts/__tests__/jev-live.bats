#!/usr/bin/env bats
# Live end-to-end check against the real TypeSafe API.
#
# Opt-in by design: it needs a real TYPESAFE_API_KEY and costs tokens, so CI
# and ordinary local runs skip it. Run it yourself with:
#   JEV_LIVE=1 bats plugins/jev/skills/jev/scripts/__tests__/jev-live.bats
# Everything else about the wrapper is covered offline in jev.bats.

SCRIPT="$(cd "${BATS_TEST_DIRNAME}/.." && pwd)/jev.sh"

setup() {
  [ "${JEV_LIVE:-}" = "1" ] || skip "set JEV_LIVE=1 to run the live API check"
  [ -n "${TYPESAFE_API_KEY:-}" ] || skip "TYPESAFE_API_KEY is not set"
}

@test "jev live: an urgent support ticket scores above 0.5 on a noul" {
  run "$SCRIPT" noul \
    -s "Help! My payouts have been failing for 3 days. I'm losing sales. Please help ASAP." \
    "Does this message express urgency?" \
    --true "Explicitly time-sensitive" \
    --false "No urgency expressed"
  [ "$status" -eq 0 ]
  noul=$(jq -r '.q.noul' <<<"$output")
  [ "$(jq -n --argjson n "$noul" '$n > 0.5')" = "true" ]
}

@test "jev live: a calm ticket scores below the urgent one on the same question" {
  run "$SCRIPT" noul \
    -s "Hi, whenever you get a chance, could you send me last quarter's invoice? No rush at all." \
    "Does this message express urgency?" \
    --true "Explicitly time-sensitive" \
    --false "No urgency expressed"
  [ "$status" -eq 0 ]
  noul=$(jq -r '.q.noul' <<<"$output")
  [ "$(jq -n --argjson n "$noul" '$n < 0.5')" = "true" ]
}

@test "jev live: an invalid key fails loudly instead of returning an empty answer" {
  TYPESAFE_API_KEY="sk-not-a-real-key" run "$SCRIPT" noul -s "anything" "Is this urgent?"
  [ "$status" -ne 0 ]
  [ "$status" -ne 1 ]
  [[ "$output" == *"{"* ]]
}
