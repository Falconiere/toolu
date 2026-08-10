---
name: deep-research
description: "Use when the ask is deep research that must end in a multi-source cited report — tells: \"deep research\", \"research report\", \"cited report\", \"survey the state of the art\", \"investigate thoroughly\". Brainstorms the guiding questions WITH the user and gets sign-off before any research runs, then fans out per-question researchers combining exa-search and context7. NOT for quick single-fact or API lookups (use research-agent) and NOT for local codebase questions (use deep-explore). Standalone knowledge workflow — not part of the build chain."
---

# Deep Research

A standalone knowledge workflow — outside the build chain; its reports can feed `brainstorm` or `spec`. It exists because the expensive failure in research is not weak sources — it is aiming a whole fan-out at a misunderstood question. So the target is agreed with the user first, and only then does the pipeline spend tokens.

## When this fires

The user wants a real research deliverable: multi-source, verified, cited. Not a quick fact or API lookup — that stays with `research-agent` — and not a question about this codebase — that is `deep-explore`'s job. Every run is multi-agent by design; if the fan-out isn't warranted, the ask wasn't deep research.

## How to run it

Five phases, driven from the main thread. Workers run on sonnet, synthesis stays on the frontier tier — rubric: `plugins/toolu/skills/orchestrator/references/model-routing.md`.

1. **Target brainstorm (main thread).** Restate the topic in one sentence. Decompose it into guiding questions with sub-questions — 5 is the typical fan-out, 7 the hard cap (the orchestrator skill's parallel-agent guardrail is the reason). Name the assumptions and what is out of scope.
2. **Approval gate (blocking).** Print the numbered question set in chat, then one `AskUserQuestion` call: approve-all / approve-with-edits (edits arrive as free text) / restart targeting. The tool caps options at 4, so this single-approval shape is the contract — not per-question multi-select. The rule is absolute: no research runs before approval. A restart goes back to question design; zero approved questions → abort, nothing written.
3. **Research fan-out.** One `research-agent` (sonnet) per approved question, launched in parallel. Each researcher combines both engines per its routing table — `context7` for library/API/docs-shaped questions, `exa-search` for general web, topics, and URL crawls — with native fallback inherited. Override the agent's default depth per call: ask for a 10–15 sentence synthesis, up to 8 sources, and `Claim → source URL` pairs for the 2–3 load-bearing claims.
4. **Verification wave.** One `research-agent` (sonnet) per finding set, prompted as an adversarial verifier: re-crawl up to 2 cited URLs per load-bearing claim (exa-search, native fallback) and return a per-claim verdict — confirmed / unsupported / source-unreachable. Unsupported claims are dropped or flagged in the report — never silently kept. If every researcher answered from training knowledge only (`Tools used: none`), skip the wave — nothing is crawlable — and banner the report "unverified — no live sources".
5. **Synthesis + delivery (main thread).** Merge the verified findings into the report format below, write it under `docs/research/`, and give the user a TLDR in chat.

**Failure semantics** — nothing fails silently: a researcher or verifier that returns nothing, errors, or is killed renders its section as `⚠ no findings returned` and is named as degraded in Reliability notes — never silently dropped, no automatic retry. Worst-case fan-out: 7 researchers + 7 verifiers = 14 subagents; typical ~10.

## Report format

Destination `docs/research/<slug>.md` — kebab-case slug from the topic. Create the directory if absent. On collision append a numeric suffix (`<slug>-2.md`); never overwrite an existing report unless the user explicitly asks.

```markdown
# <Topic>: Research Report

> Date: <YYYY-MM-DD> · Questions: <n> approved · Scope: <one line>

## TL;DR
<the answer in ≤5 bullets>

## <Guiding question 1>
<verified synthesis; flagged claims marked ⚠ unverified>

## Sources
- <title> — <url>

> Reliability notes: <per-claim verdicts; degraded workers; the unverified banner if no live sources were reachable>
```

## What "done" looks like

A report at `docs/research/<slug>.md` with a TL;DR, one section per approved question, real source URLs, and Reliability notes carrying the verification verdicts — plus the TLDR in chat. If the gate never approved a question set, "done" is: nothing ran and nothing was written.
