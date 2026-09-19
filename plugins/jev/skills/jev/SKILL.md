---
name: jev
description: Use when development needs a bounded semantic decision over supplied evidence — classify, rank, rate, judge, route, or pick one of concrete candidates — including brainstorm, spec, spec review, plan, plan review, execution, and review.
---

# Jev — Typed Judgments

Ask TypeSafe's Jev model a **typed** question about some state and get an answer
a script can branch on: a probability, a chosen option, or a score on your own
levels. Code consumes the result directly.

## Workflow rule

At each development stage, identify whether a bounded semantic judgment over
available evidence would change the next action. **If and only if it would,
and Jev is available, you MUST call it.** Use it during brainstorm to compare
concrete candidates against stated preferences; during spec/review to check
requirement wording or requirement/evidence alignment; during plan/review to
check semantic step coverage; during execution to triage supplied findings.

Keep architecture, code correctness, exact checks, arithmetic, and test verdicts
with the agent and deterministic tools. Do not call Jev merely because a stage
started. Reuse prior judgments while evidence and question meanings remain
unchanged. Record the question, answer, and decision impact where useful.

If the plugin, credentials, or service is unavailable, state the limitation and
continue with an explicit reasoning/evidence fallback. Never fabricate a result
or make a Jev answer substitute for required tests, review, or authorization.

**Trigger phrases:** classify this, rank these, rate this, how severe, which one
of these, does this count as, route this, judge whether

## CLI Tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
# Codex
"${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/jev/jev.sh" <command> [options]
# Claude Code
"${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/jev/jev.sh" <command> [options]
```

Choose the line for the active host. Ordinary shell calls do not inherit
plugin lifecycle variables, so never collapse these into one ambiguous
fallback. Use the published path; plugin-root variables are lifecycle-only.

Repo-checkout fallback (for tests/dev when the plugin is not installed): `plugins/jev/skills/jev/scripts/jev.sh`.

Requires `TYPESAFE_API_KEY` in the environment (get one at
`https://console.typesafe.ai/settings/keys`). Never read it from a `.env` file.

```
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

### Examples

```bash
# Is this diff behavioral or cosmetic?
jev.sh noul -s @/tmp/diff.txt "Does this diff change runtime behavior?" \
  --true "Logic, control flow, or output changes" \
  --false "Formatting, comments, or renames only"
# {"q":{"type":"noul","noul":0.93}}

# Route a support ticket.
jev.sh choice -s @ticket.json "Which team should handle this?" \
  -o billing="Payments, invoicing, refunds" \
  -o technical="Bugs, outages, integrations" \
  -o sales="Pricing, upgrades, new accounts"
# {"q":{"type":"choice","choice":"technical","probabilities":{...},"confidence":0.82}}

# Rate severity on your own levels.
jev.sh score -s @finding.md "How severe is this finding?" \
  -l "Cosmetic" -l "Should fix" -l "Blocks release"

# Many questions, ONE call — the cheapest way to ask several things.
jev.sh ask questions.json -s @ticket.json
```

## How to use it well

1. **Batch.** Jev ingests the state once and evaluates every question in
   parallel. Independent questions belong in one `ask` call, not N calls —
   including speculative ones your code may discard.
2. **Put the meaning in the question.** `--id` is for your code; the model never
   sees it. Instructions and criteria carry the whole judgment. `ask` takes its
   ids from the payload's own keys, so `--id` does not apply there.
3. **Ask one narrow judgment per question.** Split independent dimensions;
   don't fuse two decisions into one.
4. **Branch on the number, not on vibes.** Pick thresholds against your own
   data. For `choice`/`score`, `confidence` describes how concentrated the
   distribution is — not whether the answer is right.
5. **Give it a no-match option** when nothing may fit; the model cannot pick an
   option you left out.
6. **Keep code in code.** Exact lookups, arithmetic, and known rules stay in the
   script. Jev supplies only the semantic call.
7. **Handle uncertainty.** A Noul near 0.5 is uncertain, not medium severity.
   Investigate or fall back when uncertainty matters. For graded ranking, use
   comparable per-candidate Scores, not Choice probabilities as absolute scores.
8. **Measure.** Use `--raw` to retain actual model and token usage when evaluating
   efficiency; pin `--model` when thresholds depend on a tested version. Extra
   questions still cost tokens. Read the live API and relevant primitive or
   cookbook before designing new question formats.

## Constraints

| Constraint | Detail |
|---|---|
| Input | Text only — string, JSON object, or array. Pre-process anything else. |
| Context | 64k tokens per request; 32k for `state` plus the longest question. Slice large files before sending. |
| Errors | Up to three attempts for timeouts and HTTP `408/429/500/502/503/504/529`, with 1s then 2s backoff. Numeric `Retry-After` up to 60s is honored; longer waits surface the error. Other HTTP errors are not retried. |
| Exit codes | `1` usage/config error or invalid response, `22` HTTP error (body on stderr), `28` timeout (`JEV_TIMEOUT`, default 60s per attempt). |
| Output | Default prints `.answers` compactly; `--raw` adds `model` and token `usage`. |

Live docs: `https://docs.typesafe.ai/llms.txt` (append `.md` to any page path).
