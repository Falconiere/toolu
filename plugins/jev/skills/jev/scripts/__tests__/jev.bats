#!/usr/bin/env bats
# Tests for the jev wrapper: request construction, output projection, and the
# local guards that must never reach the network.
#
# The `curl` stub in the shared helper records argv to $CURL_LOG and replies
# with $CURL_STUB_BODY, so every assertion is made on the exact bytes the
# unmodified script would have sent or printed.

load "${BATS_TEST_DIRNAME}/../../../../../../tooling/testdata/bats/search-helpers"

# The API reference's own example response.
RESPONSE='{"model":"jev-1.13.0","answers":{"q":{"type":"noul","noul":0.92}},"usage":{"input_tokens":312,"output_tokens":48}}'

# A real support ticket, the docs' running example.
TICKET="Help! My payouts have been failing for 3 days."

setup() {
  setup_sandbox jev jev.sh
  export TYPESAFE_API_KEY="test-typesafe-key-123"
}

teardown() {
  teardown_sandbox
  unset TYPESAFE_API_KEY JEV_TIMEOUT
}

# body_json — the JSON the script handed to curl -d, pulled back out of the log.
# The script sends a compact single-line body, so it is the log's only { line.
body_json() {
  awk '/^\{/ { print; exit }' "$CURL_LOG"
}

@test "jev: missing TYPESAFE_API_KEY exits 1 with a clear error and sends nothing" {
  unset TYPESAFE_API_KEY
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Does this convey urgency?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"TYPESAFE_API_KEY unset"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: empty TYPESAFE_API_KEY is treated as unset" {
  export TYPESAFE_API_KEY=""
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Does this convey urgency?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"TYPESAFE_API_KEY unset"* ]]
}

@test "jev: no command prints the usage banner and exits 1" {
  run "$TOOL_DIR/jev.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"jev.sh <command> [options]"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: unknown command exits 1 and names it" {
  run "$TOOL_DIR/jev.sh" translate -s "$TICKET" "what"
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown command: translate"* ]]
}

@test "jev: noul POSTs to the systemone endpoint with a Bearer key" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Does this convey urgency?"
  [ "$status" -eq 0 ]
  grep -qx 'https://api.typesafe.ai/v1/systemone' "$CURL_LOG"
  grep -qx 'Authorization: Bearer test-typesafe-key-123' "$CURL_LOG"
  grep -qx 'Content-Type: application/json' "$CURL_LOG"
}

@test "jev: noul body carries jev-latest, the noul type, and the instructions verbatim" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Does this convey urgency?"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.model' <<<"$body")" = "jev-latest" ]
  [ "$(jq -r '.questions.q.type' <<<"$body")" = "noul" ]
  [ "$(jq -r '.questions.q.instructions' <<<"$body")" = "Does this convey urgency?" ]
  [ "$(jq -r '.state' <<<"$body")" = "$TICKET" ]
}

@test "jev: --true/--false become the noul criteria" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Does this convey urgency?" \
    --true "Explicitly time-sensitive" --false "No urgency expressed"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions.q.criteria."true"' <<<"$body")" = "Explicitly time-sensitive" ]
  [ "$(jq -r '.questions.q.criteria."false"' <<<"$body")" = "No urgency expressed" ]
}

@test "jev: only --true sends only the true side; neither flag omits criteria entirely" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --true "Time-sensitive"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions.q.criteria | keys | join(",")' <<<"$body")" = "true" ]

  : > "$CURL_LOG"
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$(jq 'has("criteria")' <<<"$(jq -c '.questions.q' <<<"$(body_json)")")" = "false" ]
}

@test "jev: --id names the question, defaulting to q" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --id is_urgent
  [ "$status" -eq 0 ]
  [ "$(jq -r '.questions | keys | join(",")' <<<"$(body_json)")" = "is_urgent" ]
}

@test "jev: missing --state exits 1 before any request" {
  run "$TOOL_DIR/jev.sh" noul "Does this convey urgency?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"--state is required"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: --state @missing-file exits 1 before any request" {
  run "$TOOL_DIR/jev.sh" noul -s "@$SANDBOX/nope.txt" "Urgent?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"cannot read"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: --state @file with a JSON record keeps it structured" {
  printf '%s\n' '{"subject":"payouts failing","messages":[{"text":"3 days now"}]}' > "$SANDBOX/ticket.json"
  run "$TOOL_DIR/jev.sh" noul -s "@$SANDBOX/ticket.json" "Urgent?"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.state | type' <<<"$body")" = "object" ]
  [ "$(jq -r '.state.messages[0].text' <<<"$body")" = "3 days now" ]
}

@test "jev: --state @file holding plain text is sent as a string" {
  printf '%s\n' "$TICKET" > "$SANDBOX/ticket.txt"
  run "$TOOL_DIR/jev.sh" noul -s "@$SANDBOX/ticket.txt" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.state | type' <<<"$(body_json)")" = "string" ]
}

@test "jev: a file holding a bare JSON scalar is still sent as a string" {
  printf '123\n' > "$SANDBOX/count.txt"
  run "$TOOL_DIR/jev.sh" noul -s "@$SANDBOX/count.txt" "Is this a large number?"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.state | type' <<<"$(body_json)")" = "string" ]
}

@test "jev: --state - reads the state from stdin" {
  run bash -c "printf '%s' 'Payouts have been failing for 3 days' | '$TOOL_DIR/jev.sh' noul -s - 'Urgent?'"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.state' <<<"$(body_json)")" = "Payouts have been failing for 3 days" ]
}

@test "jev: default output prints just the answers; --raw prints the whole body" {
  export CURL_STUB_BODY="$RESPONSE"
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$output" = '{"q":{"type":"noul","noul":0.92}}' ]

  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --raw
  [ "$status" -eq 0 ]
  [ "$(jq -r '.usage.input_tokens' <<<"$output")" = "312" ]
  [ "$(jq -r '.model' <<<"$output")" = "jev-1.13.0" ]
}

@test "jev: a response without answers prints null rather than inventing one" {
  export CURL_STUB_BODY='{"model":"jev-1.13.0"}'
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$output" = "null" ]
}

# ── choice ──────────────────────────────────────────────────

@test "jev: choice sends the options as a criteria object" {
  printf '%s\n' '{"subject":"payouts failing","body":"3 days now, losing sales"}' > "$SANDBOX/ticket.json"
  run "$TOOL_DIR/jev.sh" choice -s "@$SANDBOX/ticket.json" "Which team should handle this?" \
    -o billing="Payments, invoicing, refunds" \
    -o technical="Bugs, outages, integrations" \
    -o sales="Pricing, upgrades, new accounts"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions.q.type' <<<"$body")" = "choice" ]
  [ "$(jq -r '.questions.q.criteria | keys_unsorted | join(",")' <<<"$body")" = "billing,technical,sales" ]
  [ "$(jq -r '.questions.q.criteria.technical' <<<"$body")" = "Bugs, outages, integrations" ]
}

@test "jev: choice with a single option exits 1 and sends nothing" {
  run "$TOOL_DIR/jev.sh" choice -s "$TICKET" "Which team?" -o billing="Payments"
  [ "$status" -eq 1 ]
  [[ "$output" == *"at least 2 options"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: a bare -o KEY sends a null description" {
  run "$TOOL_DIR/jev.sh" choice -s "$TICKET" "Which team?" -o billing -o technical
  [ "$status" -eq 0 ]
  [ "$(jq -r '.questions.q.criteria.billing' <<<"$(body_json)")" = "null" ]
}

@test "jev: a repeated option key keeps its position and takes the last value" {
  run "$TOOL_DIR/jev.sh" choice -s "$TICKET" "Which team?" \
    -o billing="first" -o technical="Bugs" -o billing="second"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions.q.criteria | keys_unsorted | join(",")' <<<"$body")" = "billing,technical" ]
  [ "$(jq -r '.questions.q.criteria.billing' <<<"$body")" = "second" ]
}

@test "jev: choice without options exits 1 before any request" {
  run "$TOOL_DIR/jev.sh" choice -s "$TICKET" "Which team?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"at least 2 options"* ]]
  [ ! -s "$CURL_LOG" ]
}

# ── score ───────────────────────────────────────────────────

@test "jev: score sends the levels as an ordered criteria array, state from stdin" {
  run bash -c "printf '%s' '$TICKET' | '$TOOL_DIR/jev.sh' score -s - 'How frustrated is the customer?' -l Calm -l Frustrated -l 'Very angry'"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions.q.type' <<<"$body")" = "score" ]
  [ "$(jq -c '.questions.q.criteria' <<<"$body")" = '["Calm","Frustrated","Very angry"]' ]
  [ "$(jq -r '.state' <<<"$body")" = "$TICKET" ]
}

@test "jev: score with a single level exits 1 and sends nothing" {
  run "$TOOL_DIR/jev.sh" score -s "$TICKET" "How frustrated?" -l Calm
  [ "$status" -eq 1 ]
  [[ "$output" == *"at least 2 levels"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: score answers print through the default projection" {
  export CURL_STUB_BODY='{"model":"jev-1.13.0","answers":{"q":{"type":"score","score":1.6,"legend":{"0":"Calm","1":"Frustrated","2":"Very angry"},"probabilities":{"0":0.05,"1":0.3,"2":0.65},"confidence":0.78}},"usage":{"input_tokens":312,"output_tokens":48}}'
  run "$TOOL_DIR/jev.sh" score -s "$TICKET" "How frustrated?" -l Calm -l "Very angry"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.q.score' <<<"$output")" = "1.6" ]
  [ "$(jq -r '.q.confidence' <<<"$output")" = "0.78" ]
  [[ "$output" != *"usage"* ]]
}

@test "jev: an unknown option exits 1 without sending a request" {
  run "$TOOL_DIR/jev.sh" score -s "$TICKET" "How frustrated?" -l Calm -l Angry --weight 3
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown option: --weight"* ]]
  [ ! -s "$CURL_LOG" ]
}

# ── ask (fan-out) ───────────────────────────────────────────

# A real fan-out payload: three typed questions over one support ticket.
write_questions() {
  cat > "$SANDBOX/questions.json" <<'JSON'
{
  "is_urgent": { "type": "noul", "instructions": "Does this convey urgency?" },
  "department": {
    "type": "choice",
    "instructions": "Which team should handle this?",
    "criteria": { "billing": "Payments, invoicing, refunds", "technical": "Bugs, outages, integrations" }
  },
  "frustration": {
    "type": "score",
    "instructions": "How frustrated is the customer?",
    "criteria": ["Calm", "Frustrated", "Very angry"]
  }
}
JSON
}

@test "jev: ask sends the questions map verbatim in one request" {
  write_questions
  printf '%s\n' '{"subject":"payouts failing","messages":[{"text":"3 days now"}]}' > "$SANDBOX/ticket.json"
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/questions.json" -s "@$SANDBOX/ticket.json"
  [ "$status" -eq 0 ]
  body=$(body_json)
  [ "$(jq -r '.questions | keys_unsorted | join(",")' <<<"$body")" = "is_urgent,department,frustration" ]
  [ "$(jq -c '.questions' <<<"$body")" = "$(jq -c '.' "$SANDBOX/questions.json")" ]
  [ "$(jq -r '.state | type' <<<"$body")" = "object" ]
  # One state, one call.
  [ "$(grep -c '^https://api.typesafe.ai/v1/systemone$' "$CURL_LOG")" -eq 1 ]
}

@test "jev: ask prints every answer under its own id" {
  write_questions
  export CURL_STUB_BODY='{"model":"jev-1.13.0","answers":{"is_urgent":{"type":"noul","noul":0.92},"department":{"type":"choice","choice":"technical","probabilities":{"billing":0.15,"technical":0.85},"confidence":0.82}},"usage":{"input_tokens":312,"output_tokens":48}}'
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/questions.json" -s "$TICKET"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.is_urgent.noul' <<<"$output")" = "0.92" ]
  [ "$(jq -r '.department.choice' <<<"$output")" = "technical" ]
}

@test "jev: ask reads the questions from stdin" {
  write_questions
  run bash -c "'$TOOL_DIR/jev.sh' ask - -s '$TICKET' < '$SANDBOX/questions.json'"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.questions | length' <<<"$(body_json)")" = "3" ]
}

@test "jev: ask refuses a payload that is not a JSON object" {
  printf '%s\n' '[1,2]' > "$SANDBOX/bad.json"
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/bad.json" -s "$TICKET"
  [ "$status" -eq 1 ]
  [[ "$output" == *"must be a JSON object"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: ask refuses an empty questions object" {
  printf '%s\n' '{}' > "$SANDBOX/empty.json"
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/empty.json" -s "$TICKET"
  [ "$status" -eq 1 ]
  [[ "$output" == *"must not be empty"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: ask refuses invalid JSON" {
  printf '%s\n' '{"is_urgent": ' > "$SANDBOX/broken.json"
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/broken.json" -s "$TICKET"
  [ "$status" -eq 1 ]
  [[ "$output" == *"invalid JSON in"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: ask on a missing file exits 1 before any request" {
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/nope.json" -s "$TICKET"
  [ "$status" -eq 1 ]
  [[ "$output" == *"cannot read"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: questions and state cannot both read stdin" {
  run "$TOOL_DIR/jev.sh" ask - -s -
  [ "$status" -eq 1 ]
  [[ "$output" == *"only one option can read stdin"* ]]
  [ ! -s "$CURL_LOG" ]
}

# ── model + transport flags ─────────────────────────────────

@test "jev: -m pins a model version instead of the alias" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" -m jev-1.13.0
  [ "$status" -eq 0 ]
  [ "$(jq -r '.model' <<<"$(body_json)")" = "jev-1.13.0" ]
}

@test "jev: curl carries the transient-retry flags and the default 60s timeout" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  grep -qx -- '--retry' "$CURL_LOG"
  grep -qx -- '2' "$CURL_LOG"
  grep -qx -- '--max-time' "$CURL_LOG"
  grep -qx -- '60' "$CURL_LOG"
}

@test "jev: JEV_TIMEOUT overrides the curl timeout" {
  export JEV_TIMEOUT=5
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  grep -qx -- '--max-time' "$CURL_LOG"
  grep -qx -- '5' "$CURL_LOG"
}
