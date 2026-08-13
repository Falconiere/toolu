---
name: orchestrator
description: "Use when a task is broad or multi-step and should be delegated across subagents rather than run inline — exploration, codebase-wide searches, parallel builds, migrations, audits, reviews. Teaches the MAIN thread to delegate well: when to spawn a subagent vs edit inline, which agent for which job, how to parallelize independent work, how to keep main context lean, and which model tier fits each job. Tells: \"orchestrate this\", \"delegate this\", \"break this down\", \"coordinate subagents\", \"this is a big task\", or a UserPromptSubmit nudge flagged the prompt as broad/multi-step."
---

# Orchestrator

The main thread owns orchestration. Subagents do bounded work and return concise
results; they do not recursively delegate unless the task explicitly requires a
nested workflow. Use [the host mapping](../../workflows/host-mapping.md) for the
active delegation, user-input, and thread-control interfaces.

**Trigger phrases:** orchestrate this, delegate this, break this down, coordinate subagents, fan this out, this is a big/multi-step task.

## The core loop

1. **Scope inline first.** Cheap discovery the main thread should just do: list the files, find the entry points, read the one file you already know. Don't delegate a single-fact lookup.
2. **Decompose.** Split the task into independent units of work. Independent units run in parallel; dependent ones pipeline.
3. **Delegate the heavy work.** Each unit goes to a subagent with a tight prompt and a clear return contract ("return a file:line table", "return the verdict + why", "return a 5-line summary").
4. **Synthesize.** The main thread holds the conclusions, decides the next step, and edits when the work is trivial and single-file.

## Delegate vs do inline

| Delegate to a subagent | Do inline (main thread) |
|---|---|
| Broad/fan-out search across many files or naming conventions | A read of a file you already know |
| Reading many files to answer one question | A single-fact grep where you know the symbol |
| Independent work that can run in parallel | A trivial single-file edit |
| Review / audit of a diff or subsystem | Final synthesis + the decision |
| Anything that returns a lot of bytes you only need the conclusion of | Anything where delegation overhead > the work |

Rule of thumb: if the answer means reading across several files and you only need the conclusion, **delegate it and keep the conclusion, not the file dumps.** Once delegated, don't also run it yourself — wait for the result.

## Which agent for which job

Prefer a **tier-pinned** agent when one fits — its frontmatter fixes the model, so routing can't be forgotten:

- **`toolu:quick-task` / Codex `quick-task`** — mechanical lookups and listings.
- **`toolu:deep-explore` / Codex `deep-explore`** — structural exploration.
- **`toolu:research-agent` / Codex `research-agent`** — external research.
- **`toolu:implementer` / Codex `implementer`** — one bounded plan step and tests.
- **`toolu:architect` / Codex `architect`** — design and synthesis, read-only.
- **`Explore`** — broad read-only fan-out search when you need the conclusion, not file dumps.
- **`Plan`** — design an implementation strategy for a non-trivial change.
- **`general-purpose`** — multi-step research/execution that doesn't fit a specific agent; set `model:` yourself.
- **`caveman:cavecrew-investigator` / `-builder` / `-reviewer`** — when the caveman plugin is installed: compressed-output locate / bounded 1–2 file edit / diff review. Output is ~60% smaller, so main context lasts longer.

Carry the session mandates into every subagent prompt (comemory recall/save, ast-grep first). Delegation never exempts the work.

## Parallelize independent work

Launch independent subagents in **one message with multiple tool calls** so they run concurrently — not one-at-a-time. Dependent steps wait; independent steps don't. A four-way independent search done serially wastes three-quarters of the wall-clock.

## Keep main context lean

The expensive, recurring cost in a long session is **input tokens re-sent every turn** (see the token-efficiency report). Two rules follow:

- **Return conclusions, not bytes.** A subagent may read 50k tokens but should return a 1–2k-token distilled answer. The detailed context stays isolated in the subagent and never re-enters — or re-caches into — the main thread.
- **Prefer compact return formats** (tables, file:line lists) over prose dumps.

## Model tiers

Route on the **class of work**, not phrasing. Select the matching preconfigured
agent or pass the active host's explicit model and reasoning settings. Omitting
routing inherits host defaults, which may be inappropriate for the task.

| Class | Claude default | Codex default | Belongs here |
|---|---|---|---|
| mechanical | `haiku` | Luna / medium | lookups, listings, formatting, one command |
| exploration | `sonnet` | Terra / medium | read-only search across many files |
| implementation | `sonnet` | Terra / medium | a bounded decided edit + tests |
| review | `sonnet` | Terra / high | diff review, audits |
| synthesis | `opus` | Sol / high | reconciling findings |
| architecture | `opus` | Sol / high | design and hard-to-reverse calls |

Escalate one tier on any of: **hard to reverse**, **cross-cutting**, **the how isn't decided**, **must weigh alternatives**. De-escalate when the task is bounded and has one verifiable answer. Deciding and doing are different classes — one task often splits across two tiers.

A subagent that returns `ESCALATE: <reason>` should be re-run one tier up with that reason in the prompt; never re-run the same tier hoping for a better roll.

Full rubric (signals, worked examples, per-step plan tiers, config remap): `plugins/toolu/skills/orchestrator/references/model-routing.md`. Remap any class in `toolu.config.json` under `models`.

## Fan-out budget guardrail

Subagents multiply token spend (multi-agent runs can cost ~15× a single thread; unmanaged fan-out has produced four- and five-figure single-session bills). Before a large fan-out: cap the number of parallel agents to what the task needs, give each a tight scope, and prefer one well-scoped sweep over a runaway loop. If you bound coverage (top-N, sampling), say so — don't let "covered everything" hide a silent cap.
