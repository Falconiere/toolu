---
name: deep-research
description: "Use when the ask is deep research that must end in a multi-source cited report — tells: \"deep research\", \"research report\", \"cited report\", \"survey the state of the art\", \"investigate thoroughly\". Picks guiding questions, then fans out per-question researchers combining exa-search and context7. NOT for quick single-fact or API lookups — the research-agent subagent handles those internally — and NOT for local codebase questions (the deep-explore subagent). Standalone knowledge workflow — not part of the build chain."
---

# Deep Research

A standalone knowledge workflow — outside the build chain; its reports can feed `brainstorm` or `spec`. It exists because the expensive failure in research is not weak sources — it is aiming a whole fan-out at a misunderstood question. So the target is written down first, then the pipeline spends tokens. Do not wait for approval.

## When this fires

The user wants a real research deliverable: multi-source, verified, cited. Not a quick fact or API lookup — that stays with `research-agent` — and not a question about this codebase — that is `deep-explore`'s job. Every run is multi-agent by design; if the fan-out isn't warranted, the ask wasn't deep research.

## How to run it

Four phases, driven from the main thread. Workers run on sonnet, synthesis stays on the frontier tier — rubric: `plugins/toolu/skills/orchestrator/references/model-routing.md`.

1. **Target (main thread).** Restate the topic in one sentence. Decompose it into guiding questions with sub-questions — 5 is the typical fan-out, 7 the hard cap (the orchestrator skill's parallel-agent guardrail is the reason). Name the assumptions and what is out of scope. Print the numbered question set, then run it immediately without waiting for approval. Do not open the host mapping's user-choice interface ([host-mapping.md](../../workflows/host-mapping.md)).
2. **Research fan-out.** One `research-agent` (sonnet) per question, launched in parallel. Each researcher combines both engines per its routing table — `context7` for library/API/docs-shaped questions, `exa-search` for general web, topics, and URL crawls — with native fallback inherited. Override the agent's default depth per call: ask for a 10–15 sentence synthesis, up to 8 sources, and `Claim → source URL` pairs for the 2–3 load-bearing claims.
3. **Verification wave.** One `research-agent` (sonnet) per finding set, prompted as an adversarial verifier: re-crawl up to 2 cited URLs per load-bearing claim (exa-search, native fallback) and return a per-claim verdict — confirmed / unsupported / source-unreachable. Unsupported claims are dropped or flagged in the report — never silently kept. If every researcher answered from training knowledge only (`Tools used: none`), skip the wave — nothing is crawlable — and banner the report "unverified — no live sources".
4. **Synthesis + delivery (main thread).** Merge the verified findings into the report format below, write it under `docs/research/`, and give the user a TLDR in chat.

**Failure semantics** — nothing fails silently: a researcher or verifier that returns nothing, errors, or is killed renders its section as `⚠ no findings returned` and is named as degraded in Reliability notes — never silently dropped, no automatic retry. Worst-case fan-out: 7 researchers + 7 verifiers = 14 subagents; typical ~10.

## Report format

Destination `docs/research/<slug>.md` — kebab-case slug from the topic. Create the directory if absent. On collision append a numeric suffix (`<slug>-2.md`); never overwrite an existing report unless the user explicitly asks.

```markdown
# <Topic>: Research Report

> Date: <YYYY-MM-DD> · Questions: <n> · Scope: <one line>

## TL;DR
<the answer in ≤5 bullets>

## <Guiding question 1>
<verified synthesis; flagged claims marked ⚠ unverified>

## Sources
- <title> — <url>

> Reliability notes: <per-claim verdicts; degraded workers; the unverified banner if no live sources were reachable>
```

## What "done" looks like

A report at `docs/research/<slug>.md` with a TL;DR, one section per guiding question, real source URLs, and Reliability notes carrying the verification verdicts — plus the TLDR in chat. If targeting produced zero questions, "done" is: nothing ran and nothing was written.
