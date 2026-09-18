# jev — Typed Judgments from TypeSafe

**Type:** Knowledge | **Version:** 5.3.0 | **Standalone** (no dependencies)

Ask TypeSafe's Jev model a **typed** question about some state and get an answer code can branch on — a probability, a chosen option, or a score on your own levels — through a skill plus a bash REST wrapper.

## Install

```text
/plugin install jev@toolu
```

Set `TYPESAFE_API_KEY` in the environment for the wrapper to authenticate
(keys: `https://console.typesafe.ai/settings/keys`). It is never read from a
`.env` file. `JEV_TIMEOUT` overrides the 60-second curl timeout.

## What It Provides

### `jev` Skill

Always active. The typed-judgment protocol: when to ask Jev instead of spending
a reasoning turn, how to shape the question, and how to read the answer.
Triggers on classify, rank, rate, judge, route, or pick-one-of work.

### `jev.sh` Wrapper

`POST https://api.typesafe.ai/v1/systemone` with `curl` and `jq` — no SDK. The
SessionStart hook republishes it at `<config-dir>/jev/jev.sh` every session, so
the agent's shell can reach it without plugin lifecycle variables.

```text
jev.sh <command> [options]

Commands:
  noul   <instructions>    Yes/no judgment -> probability of yes
  choice <instructions>    Pick one option -> choice + probabilities + confidence
  score  <instructions>    Rate on ordered levels -> score + legend + confidence
  ask    <questions-json>  Many typed questions in ONE call (file path, or - for stdin)

Shared options:
  -s, --state VALUE   State to judge: literal text, @FILE, or - for stdin  [required]
  -m, --model NAME    Model (default: jev-latest)
      --id NAME       Question id in the answer map (noul/choice/score; default: q)
      --raw           Print the whole response body instead of just .answers

noul:    --true DESC / --false DESC   what a yes / a no means
choice:  -o, --option KEY=DESC        repeatable, at least 2 (bare -o KEY sends no description)
score:   -l, --level DESC             repeatable, at least 2, lowest level first
```

## Usage Examples

### Yes/no with a probability

```bash
jev.sh noul -s @/tmp/diff.txt "Does this diff change runtime behavior?" \
  --true "Logic, control flow, or output changes" \
  --false "Formatting, comments, or renames only"
# {"q":{"type":"noul","noul":0.93}}
```

### Pick one, with the full distribution

```bash
jev.sh choice -s @ticket.json "Which team should handle this?" \
  -o billing="Payments, invoicing, refunds" \
  -o technical="Bugs, outages, integrations" \
  -o sales="Pricing, upgrades, new accounts"
# {"q":{"type":"choice","choice":"technical","probabilities":{...},"confidence":0.82}}
```

### Rate on your own levels

```bash
jev.sh score -s @finding.md "How severe is this finding?" \
  -l "Cosmetic" -l "Should fix" -l "Blocks release"
```

### Fan out — many questions, one call

```bash
cat > questions.json <<'JSON'
{
  "is_urgent":   { "type": "noul",   "instructions": "Does this convey urgency?" },
  "department":  { "type": "choice", "instructions": "Which team should handle this?",
                   "criteria": { "billing": "Payments and refunds", "technical": "Bugs and outages" } },
  "frustration": { "type": "score",  "instructions": "How frustrated is the customer?",
                   "criteria": ["Calm", "Frustrated", "Very angry"] }
}
JSON

jev.sh ask questions.json -s @ticket.json
```

Jev ingests the state once and answers every question in parallel, so a fan-out
call costs far less than one request per question — speculative questions your
code may discard included.

### Branch on the answer

```bash
urgency=$(jev.sh noul -s @ticket.txt "Is this urgent?" | jq -r '.q.noul')
if (( $(echo "$urgency > 0.8" | bc -l) )); then page_oncall; fi
```

## Behavior and Limits

| Concern | Detail |
|---|---|
| State | Literal text stays a string. `@FILE` and `-` are sent as structured JSON only when they parse as an object or array; anything else, including a bare scalar, is sent as a string. |
| Input | Text only — string, JSON object, or array. Pre-process anything else. |
| Context | 64k tokens per request; 32k for `state` plus the longest question. Slice large files first. |
| Output | Default prints `.answers` compactly. `--raw` adds `model` and token `usage`. A response with no answers prints `null` rather than inventing one. |
| Errors | `401` bad key, `422` malformed request, `429` rate limit, `529` overloaded. curl retries the transient set twice with a 1s delay; `529` is outside that set and surfaces immediately. |
| Exit codes | `1` local usage or config error (nothing sent), `22` HTTP error with the API's body on stderr, `28` timeout. |
| Confidence | For `choice`/`score`, `confidence` describes how concentrated the distribution is — not whether the answer is right. Tune thresholds against your own data. |

## Tests

```bash
bats plugins/jev/skills/jev/scripts/__tests__/jev.bats   # offline, curl stubbed at the process boundary
bats plugins/jev/hooks/__tests__                          # SessionStart publishing

JEV_LIVE=1 TYPESAFE_API_KEY=… \
  bats plugins/jev/skills/jev/scripts/__tests__/jev-live.bats   # real API, opt-in
```

Live docs: `https://docs.typesafe.ai/llms.txt` (append `.md` to any page path).
