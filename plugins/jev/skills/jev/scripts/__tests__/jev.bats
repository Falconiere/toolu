#!/usr/bin/env bats
# Exercise the unmodified CLI with real curl against a loopback HTTPS server.
load http-helpers

# A real support ticket, the docs' running example.
TICKET="Help! My payouts have been failing for 3 days."

setup() { setup_http; }
teardown() { teardown_http; }

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

@test "jev: credential line breaks are rejected before a request" {
  for separator in $'\r' $'\n'; do
    export TYPESAFE_API_KEY="test-key${separator}X-Injected: value"
    run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
    [ "$status" -eq 1 ]
    [[ "$output" == *"TYPESAFE_API_KEY must not contain line breaks"* ]]
    [[ "$output" != *X-Injected* ]]
    [ ! -s "$CURL_LOG" ]
  done
}

@test "jev: authorization stays out of curl arguments and temporary credentials are private and cleaned" {
  mkdir "$SANDBOX/tmp"
  printf '%s' '[{"status":200,"delay":2}]' > "$SANDBOX/responses.json"
  TMPDIR="$SANDBOX/tmp" "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" >"$SANDBOX/out" 2>"$SANDBOX/err" &
  CLIENT_PID=$!
  for _ in {1..100}; do
    [ -s "$CURL_LOG" ] && break
    sleep 0.02
  done
  [ -s "$CURL_LOG" ]
  argv=$(ps -axo command | awk -v marker="$SANDBOX/tmp/" 'index($0, "curl -sS") && index($0, marker)')
  [ -n "$argv" ]
  [[ "$argv" != *"$TYPESAFE_API_KEY"* ]]
  headers=("$SANDBOX/tmp"/*/auth-header)
  python3 - "${headers[0]}" <<'PY'
import stat
import sys
from pathlib import Path
p = Path(sys.argv[1])
assert stat.S_IMODE(p.stat().st_mode) == 0o600
assert stat.S_IMODE(p.parent.stat().st_mode) == 0o700
PY
  wait "$CLIENT_PID"
  unset CLIENT_PID
  [ ! -e "${headers[0]}" ]
  jq -e '.authorization == "Bearer test-typesafe-key-123"' "$CURL_LOG"
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
  jq -e '.path == "/v1/systemone" and
    .authorization == "Bearer test-typesafe-key-123" and
    .content_type == "application/json"' "$CURL_LOG"
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
  # The API reference's own example response.
  response_body '{"model":"jev-1.13.0","answers":{"q":{"type":"noul","noul":0.92}},"usage":{"input_tokens":312,"output_tokens":48}}'
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$output" = '{"q":{"type":"noul","noul":0.92}}' ]

  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --raw
  [ "$status" -eq 0 ]
  [ "$(jq -r '.usage.input_tokens' <<<"$output")" = "312" ]
  [ "$(jq -r '.model' <<<"$output")" = "jev-1.13.0" ]
}

@test "jev: a response without answers fails instead of returning successful null" {
  response_body '{"model":"jev-1.13.0"}'
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"invalid response"* ]]
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
  response_body '{"model":"jev-1.13.0","answers":{"q":{"type":"score","score":1.6,"legend":{"0":"Calm","1":"Frustrated","2":"Very angry"},"probabilities":{"0":0.05,"1":0.3,"2":0.65},"confidence":0.78}},"usage":{"input_tokens":312,"output_tokens":48}}'
  run "$TOOL_DIR/jev.sh" score -s "$TICKET" "How frustrated?" -l Calm -l Frustrated -l "Very angry"
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
  [ "$(wc -l < "$CURL_LOG" | tr -d ' ')" -eq 1 ]
}

@test "jev: ask prints every answer under its own id" {
  write_questions
  response_body '{"model":"jev-1.13.0","answers":{"is_urgent":{"type":"noul","noul":0.92},"department":{"type":"choice","choice":"technical","probabilities":{"billing":0.15,"technical":0.85},"confidence":0.82},"frustration":{"type":"score","score":1.6,"legend":{"0":"Calm","1":"Frustrated","2":"Very angry"},"probabilities":{"0":0.05,"1":0.3,"2":0.65},"confidence":0.78}},"usage":{"input_tokens":312,"output_tokens":48}}'
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

@test "jev: JEV_TIMEOUT bounds a slow HTTP attempt and returns 28" {
  printf '%s' '[{"status":200,"delay":0.3}]' > "$SANDBOX/responses.json"
  export JEV_TIMEOUT=0.05
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 28 ]
}

@test "jev: --help prints the usage banner and exits 1" {
  run "$TOOL_DIR/jev.sh" --help
  [ "$status" -eq 1 ]
  [[ "$output" == *"jev.sh <command> [options]"* ]]
  [[ "$output" == *"Many typed questions in ONE call"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: an option missing its value exits 1 before any request" {
  run "$TOOL_DIR/jev.sh" noul "Urgent?" -s
  [ "$status" -eq 1 ]
  [[ "$output" == *"--state needs a value"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: ask rejects --id instead of silently ignoring it" {
  write_questions
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/questions.json" -s "$TICKET" --id mine
  [ "$status" -eq 1 ]
  [[ "$output" == *"--id does not apply"* ]]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: an empty --true is not sent as an empty criteria object" {
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --true ""
  [ "$status" -eq 0 ]
  [ "$(jq '.questions.q | has("criteria")' <<<"$(body_json)")" = "false" ]
}

@test "jev: 529 and 429 retry to one valid answer without leaked error bodies" {
  printf '%s' '[{"status":529,"body":"{\"error\":\"overloaded\"}"},{"status":429,"retry_after":1,"body":"{\"error\":\"rate limited\"}"},{"status":200}]' > "$SANDBOX/responses.json"
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 0 ]
  [ "$output" = '{"q":{"type":"noul","noul":0.92}}' ]
  [ "$(wc -l < "$CURL_LOG" | tr -d ' ')" -eq 3 ]
}

@test "jev: persistent overload stops after three attempts with HTTP status 22" {
  printf '%s' '[{"status":529,"body":"{\"error\":\"overloaded\"}"}]' > "$SANDBOX/responses.json"
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 22 ]
  [[ "$output" == *"overloaded"* ]]
  [ "$(wc -l < "$CURL_LOG" | tr -d ' ')" -eq 3 ]
}

@test "jev: 401 is not retried and error JSON goes only to stderr" {
  printf '%s' '[{"status":401,"body":"{\"error\":\"invalid key\"}"}]' > "$SANDBOX/responses.json"
  run bash -c '"$1" noul -s ticket "Urgent?" >"$2"' _ "$TOOL_DIR/jev.sh" "$SANDBOX/stdout"
  [ "$status" -eq 22 ]
  [[ "$output" == *"invalid key"* ]]
  [ ! -s "$SANDBOX/stdout" ]
  [ "$(wc -l < "$CURL_LOG" | tr -d ' ')" -eq 1 ]
}

@test "jev: missing requested answers and wrong answer types are rejected" {
  response_body '{"answers":{"other":{"type":"noul","noul":0.9}}}'
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?"
  [ "$status" -eq 1 ]
  [[ "$output" == *"invalid response"* ]]
  response_body '{"answers":{"q":{"type":"choice","choice":"yes"}}}'
  run "$TOOL_DIR/jev.sh" noul -s "$TICKET" "Urgent?" --raw
  [ "$status" -eq 1 ]
}

@test "jev: JSON streams in state stay text instead of dropping all but the last record" {
  printf '%s\n' '{"first":"one"}' '{"second":"two"}' > "$SANDBOX/records.jsonl"
  run "$TOOL_DIR/jev.sh" noul -s "@$SANDBOX/records.jsonl" "Does this mention one?"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.state | type' <<<"$(body_json)")" = string ]
  [[ "$(jq -r '.state' <<<"$(body_json)")" == *first* ]]
}

@test "jev: multiple question objects are rejected before any request" {
  printf '%s\n' '{"a":{"type":"noul","instructions":"A?"}}' '{"b":{"type":"noul","instructions":"B?"}}' > "$SANDBOX/questions.json"
  run "$TOOL_DIR/jev.sh" ask "$SANDBOX/questions.json" -s ticket
  [ "$status" -eq 1 ]
  [ ! -s "$CURL_LOG" ]
}

@test "jev: invalid distributions and incomplete response envelopes fail closed" {
  for response in \
    '{"answers":{"q":{"type":"noul","noul":0.9}}}' \
    '{"model":"jev-1.13.0","usage":{"input_tokens":1,"output_tokens":0},"answers":{"q":{"type":"choice","choice":"a","confidence":1,"probabilities":{}}}}' \
    '{"model":"jev-1.13.0","usage":{"input_tokens":1,"output_tokens":0},"answers":{"q":{"type":"choice","choice":"a","confidence":1,"probabilities":{"a":"yes","b":0}}}}' \
    '{"model":"jev-1.13.0","usage":{"input_tokens":1,"output_tokens":0},"answers":{"q":{"type":"choice","choice":"a","confidence":1,"probabilities":{"a":0.2,"b":0.3}}}}'; do
    response_body "$response"
    run "$TOOL_DIR/jev.sh" choice -s ticket 'Which?' -o a -o b
    [ "$status" -eq 1 ]
    [[ "$output" == *"invalid response"* ]]
  done
  response_body '{"model":"jev-1.13.0","usage":{"input_tokens":1,"output_tokens":0},"answers":{"q":{"type":"score","score":0.5,"confidence":0.5,"probabilities":{"0":0.5,"1":0.5},"legend":{"unrelated":"level"}}}}'
  run "$TOOL_DIR/jev.sh" score -s ticket 'How severe?' -l Low -l High
  [ "$status" -eq 1 ]
}
