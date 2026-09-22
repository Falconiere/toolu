# jev — Typed Judgments from TypeSafe

**Type:** Knowledge | **Version:** 6.5.0 | **Standalone** (no dependencies)

Ask TypeSafe's Jev model a **typed** question about some state and get an answer code can branch on — a probability, a chosen option, or a score on your own levels — through a skill plus a bash REST wrapper.

## Install

Claude Code:

```text
/plugin marketplace add Falconiere/toolu
/plugin install jev@toolu
```

Codex:

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add jev@toolu
```

Restart the host after installation. In Codex, review and trust the installed
hook through `/hooks`; installation alone does not trust hooks. Both hosts
need `curl`, `jq`, and `TYPESAFE_API_KEY` in their launch environment.

Set `TYPESAFE_API_KEY` in the environment for the wrapper to authenticate
(keys: `https://console.typesafe.ai/settings/keys`). It is never read from a
`.env` file. `JEV_TIMEOUT` overrides the 60-second timeout per attempt.

This repo's PR review workflow (`falconiere/toolu-ghactions/code-review@v8`) also
opts into Jev finding assessment with `JEV_ENABLED: 'true'` and
`JEV_MODEL_ID: typesafe/jev-1.13` (OpenRouter), separate from the in-session
`jev` plugin CLI.

## What It Provides

### `jev` Skill

Mandatory on every task containing semantic decisions: after initial exploration,
use Jev for focused judgments over gathered evidence before the decision it informs.
Reassess after new evidence, failed hypotheses, or changed requirements throughout
search, debugging, planning, execution, and review. Batch independent questions and
reuse results while evidence and question meanings stay unchanged. A task with no
semantic decision is stated as such in one sentence. Keep architecture, code
correctness, tests, and exact rules with the agent and deterministic tools.

The [problem-solving reference](../../plugins/jev/skills/jev/references/problem-solving.md)
contains executable examples with named evidence, rubrics, interpretation, and
next actions, including no-match and uncertain outcomes.

The SessionStart hook injects the full rule on startup, resume, clear, and
compaction; the UserPromptSubmit hook restates a short form on every prompt so
the rule survives long sessions. Neither calls the API. Missing prerequisites
produce one actionable fallback message at session start and silence per prompt.
Enforcement is through workflow instructions; it does not block edits or commits.
See the [workflow guidance](../../plugins/toolu/workflows/semantic-judgments.md).

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
choice:  -o, --option KEY=DESC        repeatable, 2..255 (bare -o KEY sends no description)
score:   -l, --level DESC             repeatable, 2..10, lowest level first
```

Structured `instructions` and `criteria` (objects, arrays, `null`), which the
API accepts on every question type, go through `ask`; the single-question
commands take strings.

## Problem-solving examples

Load only Setup + the needed [example](../../plugins/jev/skills/jev/references/problem-solving.md):
search (rank + existence), debugging (hypotheses + new evidence), planning
(separate preferences), or review (support/contradiction/unsupported).
[Live evaluation record](../../plugins/jev/skills/jev/evals/README.md).

## Behavior and Limits

| Concern | Detail |
|---|---|
| State | Literal text stays a string. `@FILE` and `-` are sent as structured JSON only when they parse as an object or array; anything else, including a bare scalar, is sent as a string. |
| Input | Text only — string, JSON object, or array. Pre-process anything else. English is the primary training language. |
| Limits | Choice: 2–255 options. Score: 2–10 levels. Checked locally before any request (the API rejects 11 levels with `Too many score levels`). |
| Context | 64k tokens per request; 32k for `state` plus the longest question. Slice large files first. |
| Output | Default prints `.answers` compactly. `--raw` adds `model` and token `usage`. Missing or invalid typed answers fail explicitly, including with `--raw`. |
| Errors | Up to three attempts for timeouts, connection failures, and HTTP `408`, `429`, and any `5xx` — the SDK's default retry set — with 1s then 2s backoff. `Retry-After` (seconds) or `retry-after-ms` up to 60s is honored; longer waits surface the error. `401`/`422` and other 4xx are not retried. |
| Exit codes | `1` usage/config error or invalid response, `22` HTTP error with the API's body on stderr, `28` timeout. |
| Confidence | For `choice`/`score`, `confidence` describes how concentrated the distribution is — not whether the answer is right. Tune thresholds against your own data. |

## Tests

```bash
bats plugins/jev/skills/jev/scripts/__tests__/jev.bats   # offline, real curl + loopback HTTPS
bats plugins/jev/hooks/__tests__                          # SessionStart publishing + per-prompt mandate

JEV_LIVE=1 TYPESAFE_API_KEY=… \
  bats plugins/jev/skills/jev/scripts/__tests__/jev-live.bats   # real API, opt-in
```

Live docs: `https://docs.typesafe.ai/llms.txt` (append `.md` to any page path).
