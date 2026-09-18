#!/usr/bin/env bash
set -euo pipefail

# jev — typed judgments from TypeSafe's System One API.
# Usage: ./jev.sh <command> [options]
#
# Reads TYPESAFE_API_KEY from the environment (never from .env).

command -v jq   >/dev/null 2>&1 || { echo "jev: jq required" >&2;   exit 1; }
command -v curl >/dev/null 2>&1 || { echo "jev: curl required" >&2; exit 1; }

JEV_URL="https://api.typesafe.ai/v1/systemone"
DEFAULT_MODEL="jev-latest"

TYPESAFE_API_KEY="${TYPESAFE_API_KEY:-}"
if [[ -z "$TYPESAFE_API_KEY" ]]; then
  echo "jev: TYPESAFE_API_KEY unset" >&2
  exit 1
fi

die() { echo "jev: $1" >&2; exit 1; }

usage() {
  cat >&2 <<'EOF'
jev.sh <command> [options]

Commands:
  noul   <instructions>    Yes/no judgment -> probability of yes
  choice <instructions>    Pick one option -> choice + probabilities + confidence
  score  <instructions>    Rate on ordered levels -> score + legend + confidence
  ask    <questions-json>  Many typed questions in ONE call (file path, or - for stdin)

Shared options:
  -s, --state VALUE   State to judge: literal text, @FILE, or - for stdin  [required]
  -m, --model NAME    Model (default: jev-latest)
      --id NAME       Question id in the answer map (default: q)
      --raw           Print the whole response body instead of just .answers

noul:    --true DESC / --false DESC   what a yes / a no means
choice:  -o, --option KEY=DESC        repeatable, at least 2 (bare -o KEY sends no description)
score:   -l, --level DESC             repeatable, at least 2, lowest level first
EOF
  exit 1
}

# Shared option state.
STATE_RAW=""
STATE_SET=false
STATE_FROM_STDIN=false
MODEL="$DEFAULT_MODEL"
QID="q"
RAW=false
INSTRUCTIONS=""

# state_json -> the JSON value for the request's `state` field.
# A literal string stays a string. @FILE and - are sent as structured JSON only
# when they parse as an object or an array; anything else (including a bare
# JSON scalar such as 123) is sent as a string.
state_json() {
  local text file
  case "$STATE_RAW" in
    -)
      text="$(cat)"
      ;;
    @*)
      file="${STATE_RAW#@}"
      [[ -r "$file" ]] || die "cannot read $file"
      text="$(cat "$file")"
      ;;
    *)
      printf '%s' "$STATE_RAW" | jq -Rs '.'
      return
      ;;
  esac
  if printf '%s' "$text" | jq -e 'type == "object" or type == "array"' >/dev/null 2>&1; then
    printf '%s' "$text" | jq -c '.'
  else
    printf '%s' "$text" | jq -Rs '.'
  fi
}

# claim_stdin — only one option may read stdin per invocation.
claim_stdin() {
  [[ "$STATE_FROM_STDIN" == true ]] && die "only one option can read stdin"
  STATE_FROM_STDIN=true
}

# parse_shared OPTION ARG... -> RETURNS the consumed-arg count (0 = unrecognized).
# Callers must invoke it as `consumed=0; parse_shared "$@" || consumed=$?` so the
# non-zero count is read as data instead of tripping `set -e`.
parse_shared() {
  case "$1" in
    -s|--state)
      [[ $# -ge 2 ]] || die "--state needs a value"
      STATE_RAW="$2"; STATE_SET=true
      [[ "$2" == "-" ]] && claim_stdin
      return 2;;
    -m|--model)
      [[ $# -ge 2 ]] || die "--model needs a value"
      MODEL="$2"; return 2;;
    --id)
      [[ $# -ge 2 ]] || die "--id needs a value"
      QID="$2"; return 2;;
    --raw)
      RAW=true; return 1;;
  esac
  return 0
}

require_state() { [[ "$STATE_SET" == true ]] || die "--state is required"; }
require_instructions() { [[ -n "$INSTRUCTIONS" ]] || usage; }

jev_post() {
  curl -sS --fail-with-body \
    --retry 2 --retry-delay 1 \
    --max-time "${JEV_TIMEOUT:-60}" \
    -X POST "$JEV_URL" \
    -H "Authorization: Bearer $TYPESAFE_API_KEY" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json" \
    -d "$1"
}

# evaluate QUESTIONS_JSON — send the request and print the answers.
# An HTTP failure propagates: the API's error body goes to stderr and the exit
# status is curl's own (22 for an error response, 28 for a timeout).
evaluate() {
  local questions="$1" state body out status
  # Resolved in its own statement: a failure inside state_json exits the command
  # substitution's subshell, and only an explicit check propagates that here.
  state="$(state_json)" || exit $?
  body=$(jq -nc \
    --argjson state "$state" \
    --arg model "$MODEL" \
    --argjson questions "$questions" \
    '{state: $state, model: $model, questions: $questions}')

  set +e
  out="$(jev_post "$body")"
  status=$?
  set -e
  if [[ "$status" -ne 0 ]]; then
    [[ -n "$out" ]] && printf '%s\n' "$out" >&2
    exit "$status"
  fi

  if [[ "$RAW" == true ]]; then
    printf '%s\n' "$out" | jq '.'
  else
    printf '%s\n' "$out" | jq -c '.answers'
  fi
}

# ── noul ────────────────────────────────────────────────────
cmd_noul() {
  local true_desc="" false_desc="" has_criteria=false consumed

  while [[ $# -gt 0 ]]; do
    consumed=0
    parse_shared "$@" || consumed=$?
    if [[ "$consumed" -gt 0 ]]; then shift "$consumed"; continue; fi
    case "$1" in
      --true)  [[ $# -ge 2 ]] || die "--true needs a value"
               true_desc="$2"; has_criteria=true; shift 2;;
      --false) [[ $# -ge 2 ]] || die "--false needs a value"
               false_desc="$2"; has_criteria=true; shift 2;;
      -*)      die "unknown option: $1";;
      *)       [[ -z "$INSTRUCTIONS" ]] || die "unexpected argument: $1"
               INSTRUCTIONS="$1"; shift;;
    esac
  done

  require_instructions
  require_state

  # criteria is omitted entirely when neither flag is given, and carries only
  # the side(s) actually described.
  local criteria question
  criteria=$(jq -n \
    --arg t "$true_desc" \
    --arg f "$false_desc" \
    '(if $t == "" then {} else {"true": $t} end) + (if $f == "" then {} else {"false": $f} end)')
  question=$(jq -n \
    --arg id "$QID" \
    --arg instructions "$INSTRUCTIONS" \
    --argjson criteria "$criteria" \
    --argjson has_criteria "$has_criteria" \
    '{($id): ({type: "noul", instructions: $instructions}
              + (if $has_criteria then {criteria: $criteria} else {} end))}')
  evaluate "$question"
}

# ── choice ──────────────────────────────────────────────────
cmd_choice() {
  local criteria='{}' consumed key desc

  while [[ $# -gt 0 ]]; do
    consumed=0
    parse_shared "$@" || consumed=$?
    if [[ "$consumed" -gt 0 ]]; then shift "$consumed"; continue; fi
    case "$1" in
      -o|--option)
        [[ $# -ge 2 ]] || die "--option needs a value"
        key="${2%%=*}"
        [[ -n "$key" ]] || die "--option needs a KEY (KEY=DESCRIPTION)"
        # A bare KEY sends null — the API's documented "no extra detail" form.
        # A repeated KEY keeps its original position and takes the last value.
        if [[ "$2" == *=* ]]; then
          desc="${2#*=}"
          criteria=$(jq -c --arg k "$key" --arg d "$desc" '. + {($k): $d}' <<<"$criteria")
        else
          criteria=$(jq -c --arg k "$key" '. + {($k): null}' <<<"$criteria")
        fi
        shift 2;;
      -*) die "unknown option: $1";;
      *)  [[ -z "$INSTRUCTIONS" ]] || die "unexpected argument: $1"
          INSTRUCTIONS="$1"; shift;;
    esac
  done

  require_instructions
  require_state
  [[ "$(jq 'length' <<<"$criteria")" -ge 2 ]] || die "choice needs at least 2 options"

  local question
  question=$(jq -nc \
    --arg id "$QID" \
    --arg instructions "$INSTRUCTIONS" \
    --argjson criteria "$criteria" \
    '{($id): {type: "choice", instructions: $instructions, criteria: $criteria}}')
  evaluate "$question"
}

# ── score ───────────────────────────────────────────────────
cmd_score() {
  local levels='[]' consumed

  while [[ $# -gt 0 ]]; do
    consumed=0
    parse_shared "$@" || consumed=$?
    if [[ "$consumed" -gt 0 ]]; then shift "$consumed"; continue; fi
    case "$1" in
      -l|--level)
        [[ $# -ge 2 ]] || die "--level needs a value"
        # Order is the rubric: levels stay in the order they were given.
        levels=$(jq -c --arg l "$2" '. + [$l]' <<<"$levels")
        shift 2;;
      -*) die "unknown option: $1";;
      *)  [[ -z "$INSTRUCTIONS" ]] || die "unexpected argument: $1"
          INSTRUCTIONS="$1"; shift;;
    esac
  done

  require_instructions
  require_state
  [[ "$(jq 'length' <<<"$levels")" -ge 2 ]] || die "score needs at least 2 levels"

  local question
  question=$(jq -nc \
    --arg id "$QID" \
    --arg instructions "$INSTRUCTIONS" \
    --argjson levels "$levels" \
    '{($id): {type: "score", instructions: $instructions, criteria: $levels}}')
  evaluate "$question"
}

# ── ask ─────────────────────────────────────────────────────
# Many typed questions against one state, in a single request. Jev ingests the
# state once and answers every question in parallel, so this is the cheap way
# to ask several things — including speculative ones the caller may discard.
cmd_ask() {
  local src="" consumed text questions

  while [[ $# -gt 0 ]]; do
    consumed=0
    parse_shared "$@" || consumed=$?
    if [[ "$consumed" -gt 0 ]]; then shift "$consumed"; continue; fi
    case "$1" in
      # A lone dash is the stdin source, not an option.
      -)  [[ -z "$src" ]] || die "unexpected argument: -"
          src="-"; claim_stdin; shift;;
      -*) die "unknown option: $1";;
      *)  [[ -z "$src" ]] || die "unexpected argument: $1"
          src="$1"; shift;;
    esac
  done

  [[ -n "$src" ]] || usage
  require_state

  if [[ "$src" == "-" ]]; then
    text="$(cat)"
  else
    [[ -r "$src" ]] || die "cannot read $src"
    text="$(cat "$src")"
  fi

  questions="$(printf '%s' "$text" | jq -c '.' 2>/dev/null)" || die "invalid JSON in $src"
  [[ "$(jq -r 'type' <<<"$questions")" == "object" ]] || die "questions must be a JSON object"
  [[ "$(jq 'length' <<<"$questions")" -ge 1 ]] || die "questions must not be empty"

  evaluate "$questions"
}

# ── dispatch ────────────────────────────────────────────────
[[ $# -gt 0 ]] || usage
command="$1"; shift
case "$command" in
  noul)   cmd_noul "$@";;
  choice) cmd_choice "$@";;
  score)  cmd_score "$@";;
  ask)    cmd_ask "$@";;
  -h|--help|help) usage;;
  *)      echo "jev: unknown command: $command" >&2; usage;;
esac
