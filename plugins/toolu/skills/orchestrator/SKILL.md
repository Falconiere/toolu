---
name: orchestrator
description: "Use when deciding HOW to run a large task: whether to delegate it at all, and if so how to split it. Teaches the MAIN thread that inline is the default and a subagent must earn its round trip — plus which agent for which job, how to bound a delegation so it cannot hang, how to parallelize genuinely independent work, and which model tier fits each class. Read it before fanning anything out, especially when tempted to split work that one thread could just do. Tells: \"orchestrate this\", \"delegate this\", \"break this down\", \"coordinate subagents\", \"should I use subagents\", \"this is a big task\", or a UserPromptSubmit nudge flagged the prompt as large."
---

# Orchestrator

The main thread owns orchestration — and most of the time, orchestration means
doing the work. A subagent is a tool with a price: a spawn, a prompt you have to
write, a round trip, and a summary you then have to trust. Delegation is worth it
when that price buys something (isolation of a large read, real parallelism); it
is a loss when it does not.

The failure this skill exists to prevent is not under-delegation. It is a simple
task fanned out to five agents, each waiting on the last, taking ten minutes to
do three minutes of work.

Subagents do bounded work and return concise results; they do not recursively
delegate unless the task explicitly requires a nested workflow. Use [the host
mapping](../../workflows/host-mapping.md) for the active delegation, user-input,
and thread-control interfaces.

**Trigger phrases:** orchestrate this, delegate this, break this down, coordinate subagents, fan this out, this is a big/multi-step task.

## Does this need delegating at all?

Ask before decomposing anything. Delegation pays only when **at least one** of
these is true:

1. **The output is large and you need only the conclusion.** Reading thirty files
   to answer one question — the bytes stay in the subagent, the answer comes back.
2. **Two or more units are genuinely independent** and can run at the same time.
   Genuinely: neither needs the other's result.

…**and** the work is bigger than a handful of tool calls. Below that, the round
trip costs more than doing it.

If neither holds, do it inline. That is not a failure of orchestration; it is
orchestration. Worked examples, including the cases where delegation loses:
[delegation cost](references/delegation-cost.md).

Two shapes that look like orchestration and are not:

- **A dependent chain.** A → wait → B → wait → C, where each needs the last, is
  strictly slower than doing A, B, C inline: you pay every round trip and gain no
  parallelism. Chain inline; delegate the branches.
- **Splitting to look thorough.** Three agents on one small file is not coverage,
  it is three summaries of the same thing.

## The core loop

1. **Do the cheap discovery yourself.** List the files, find the entry point, read
   the file you already know. Never delegate a single-fact lookup.
2. **Decide whether delegation pays** by the test above. Often it does not — go
   do the work.
3. **If it pays, decompose into independent units.** Independent units run in
   parallel; dependent ones you do inline.
4. **Delegate with a contract.** One answerable question per agent, a tight
   prompt, and an explicit return shape ("a file:line table", "the verdict + why",
   "a 5-line summary").
5. **Synthesize.** The main thread holds the conclusions and makes the decisions.

## Delegate vs do inline

Inline is the default, so the right-hand column is where most work belongs.

| Do inline (main thread) | Delegate to a subagent |
|---|---|
| A read of a file you already know | Broad/fan-out search across many files or naming conventions |
| A single-fact grep where you know the symbol | Reading many files to answer one question |
| An edit you can make in a few tool calls, however many files | Independent work that can genuinely run in parallel |
| A dependent chain of steps — each needs the last | Review / audit of a whole diff or subsystem |
| The final synthesis and every decision | Anything returning a lot of bytes you need only the conclusion of |
| Anything where the round trip costs more than the work | A long-running sweep you want isolated from main context |

Rule of thumb: if the answer means reading across several files and you only need the conclusion, **delegate it and keep the conclusion, not the file dumps.** Once delegated, don't also run it yourself in parallel — that pays for the work twice.

## When an agent goes quiet

Waiting is not a strategy. Every delegation is bounded before it starts, and
unbounded waiting is a bug:

- **Scope it to one answerable question.** "Find every call site of X" comes back.
  "Improve the module" does not.
- **State the return contract in the prompt.** An agent that knows it owes a
  file:line table finishes; one asked to "look into it" wanders.
- **If it does not come back, check once. Then take the work back inline and say
  so.** Report what you did instead — a silent takeover looks like the agent
  succeeded.
- **Never block indefinitely.** A subagent that has gone quiet has already cost
  more than the work; sitting on it converts a slow task into a stuck one.

An agent that returns something unusable is the same case: use it or redo the
work inline, but do not re-spawn the same prompt hoping for a better roll.

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
