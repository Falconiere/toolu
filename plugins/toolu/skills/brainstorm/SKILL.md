---
name: brainstorm
description: "Use BEFORE writing code or planning any build, feature, refactor, or behavior change whose shape isn't settled — even when the user states only WHAT they want (\"add X\", \"fix Y\"). Catch the tells: \"where do I even start\", \"this feels big\", \"help me scope it\", \"think through the approach and tradeoffs\", \"before I start coding\". Surfaces intent, constraints, prior art, and trade-offs, then records the decision. First phase of the toolu workflow. Skip mechanical work with no design question (renames, dep bumps, one-line fixes) and already-scoped features."
---

# Brainstorm

First phase of the toolu workflow. It exists to kill the most expensive mistake in software: building the wrong thing well, or the right thing on the wrong foundation. A request names a WHAT; the cost lives in the HOW. Five minutes deciding the shape here saves a day of rework downstream.

This is the toolu-native version: it carries the house conventions forward so `spec`, `plan`, `execution`, and `test` inherit a concrete, agreed design — not just a vibe.

## When this fires

Any request that implies new or changed behavior: a feature, a component, a refactor, a behavior tweak. "Add X" and "fix Y" still owe a HOW conversation — the imperative tells you the goal, not the design. If you catch yourself about to open a file or enter plan mode without an agreed approach, stop and brainstorm first.

Skip it only for genuinely mechanical work where the design is not in question — a typo, a rename, a dependency bump.

## How to run it

The goal is a written design you and the user both believe in. Get there however the problem demands; the steps below are the usual path, not a ritual.

1. **Restate the intent in one sentence and reflect it back.** If you can't, you don't understand it yet — that's the first thing to fix. A wrong restatement is cheap to correct now and ruinous to discover after implementation.
2. **Sweep the design dimensions before asking anything.** Walk the full list — intent & success, scope boundary, data & state, interface & UX shape, failure behavior, integration & blast radius, constraints, effort & horizon — and sort each one into *ask* (different answers would change what gets built), *default* (you can settle it, but say so out loud), or *N/A*. The sweep is the point: it's what stops you from asking three obvious questions and missing the one that reshapes the design. Bank: `plugins/toolu/skills/brainstorm/references/design-questions.md`.
3. **Ask in rounds, with options, until nothing material is open.** Put every *ask* dimension through `AskUserQuestion` — up to four questions per call, 2–4 concrete options each, your recommendation first and labeled, and a `preview` whenever the choice is a shape the user should see (file layout, schema, CLI surface, interface signature). Batch a round; never trickle. Then keep going: answers routinely open a fork the first round couldn't see ("persist it" → *where, and does it migrate?*), and that follow-up is convergence, not creep. Stop when a round would produce no answer that changes the design — not when you feel you've asked enough. Never ask a question that has one sane answer; state that default in the same message instead, so the user can override it without being interrogated.
4. **Find what already exists.** Search for reusable functions, patterns, and prior art before inventing (`ast-grep` / `comemory`). The best design is often "extend this thing that already works." New code is a liability you justify, not a default. This interleaves with step 3 rather than waiting for it — when a round exposes something worth looking for, go look before the next round, because an option grounded in code that already exists beats one invented at the desk.
5. **Put three or four real approaches on the table and recommend one.** Span the range — the minimal version, the conventional one, the ambitious one — rather than three variants of the same instinct; two options is a false binary unless the third is genuinely dead. Name the trade-off that actually matters for *this* problem (simplicity vs. flexibility, speed vs. correctness, blast radius vs. cleanliness), and for each option say what it forecloses, not just what it does. Offer them as options too, recommendation first. A menu with no recommendation pushes the decision back onto the user; have an opinion.
6. **Record the decision.** A short decision record: chosen approach, the why, the alternatives you rejected and why, the defaults you set without asking, and the risks still open. Save anything durable (a convention, a constraint, a non-obvious call) to memory via `agent-memory` so it outlives this conversation.

**Delegate at the right tier.** Step 4's prior-art hunt is exploration — send it to a mid-tier agent (`toolu:deep-explore`) and keep the bytes out of this thread. Step 5's trade-off call is architecture: it stays with the top tier (here, or `toolu:architect`). Never let a cheap tier settle the design, and never spend the top tier on the search that feeds it. Rubric: `plugins/toolu/skills/orchestrator/references/model-routing.md`.

**When to stop asking.** The budget is the user's attention, not a question count. Spend it on decisions where the answers diverge; spend none of it on decisions where they don't. A brainstorm that asks twelve questions across three rounds and lands the design is cheaper than one that asks two and builds the wrong shape well.

## Conventions to carry forward

These are toolu defaults the later phases enforce, so decide them here rather than discovering them mid-implementation:

- **Legible structure** — one responsibility per file, files named after what they export. Code humans and AI can both navigate without a map.
- **Test strategy** — tests colocate by language: TS in a sibling `__tests__/`, Rust in a sibling `tests/`. Real-world data only; no mock-data tests. Decide *what* you'll test against now.
- **Docs** — every module and public symbol gets a concise doc line. Required, but brief — plan for it, don't bolt it on.
- **Docs in sync** — when a change touches a user-facing surface (behavior, interfaces, CLI flags, commands, config), the prose docs that describe it stay current: README, `docs/` guides, `SKILL.md` trigger text, release notes. Distinct from the code-symbol doc line above. Decide here which surfaces this task touches so the later phases require, do, and check the update.
- **Size discipline** — default ceilings of 300 code lines per TS file / 500 per Rust file (blanks and comments excluded, per-project overridable). If the design implies a giant file, split it in the design, not after the gate complains.

## What "done" looks like

A confirmed one-sentence intent, every design dimension either answered or explicitly defaulted, an agreed approach with its trade-off named, and a decision record capturing the why and the open risks. That is the handoff to `spec` (which writes the design down) — or straight to `plan` for smaller work that doesn't warrant a written spec. If you don't have those three things, you're not done brainstorming yet.
