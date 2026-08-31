---
name: brainstorm
description: "Use BEFORE writing code or planning any build, feature, refactor, or behavior change whose shape isn't settled — even when the user states only WHAT they want (\"add X\", \"fix Y\"). Catch the tells: \"where do I even start\", \"this feels big\", \"help me scope it\", \"think through the approach and tradeoffs\", \"before I start coding\". Surfaces intent, constraints, prior art, and trade-offs, then records the decision. First phase of the toolu workflow. Skip mechanical work with no design question (renames, dep bumps, one-line fixes) and already-scoped features."
---

# Brainstorm

First phase of the toolu workflow. It exists to kill the most expensive mistake in software: building the wrong thing well, or the right thing on the wrong foundation. A request names a WHAT; the cost lives in the HOW. Five minutes deciding the shape here saves a day of rework downstream.

This is the toolu-native version: it carries the house conventions forward so `spec`, `plan`, `execution`, and `test` inherit a concrete design — not just a vibe.

## When this fires

Any request that implies new or changed behavior: a feature, a component, a refactor, a behavior tweak. "Add X" and "fix Y" still owe a HOW — the imperative tells you the goal, not the design. If you catch yourself about to open a file or enter plan mode without a settled approach, stop and brainstorm first.

Skip it only for genuinely mechanical work where the design is not in question — a typo, a rename, a dependency bump.

## How to run it

The goal is a written design the rest of the workflow can inherit. Get there however the problem demands; the steps below are the usual path, not a ritual. Do not open the host mapping's user-choice interface — pick the recommended default, state it, and proceed. The user can override by speaking up.

1. **Restate the intent in one sentence.** If you can't, you don't understand it yet — that's the first thing to fix. A wrong restatement is cheap to correct now and ruinous to discover after implementation.
2. **Sweep the design dimensions and pick a default for each.** Walk the full list — intent & success, scope boundary, data & state, interface & UX shape, failure behavior, integration & blast radius, constraints, effort & horizon — and sort each one into *default* (you can settle it; say so) or *N/A*. Never leave a material dimension open waiting for an answer. Bank: `plugins/toolu/skills/brainstorm/references/design-questions.md`.
3. **Find what already exists.** Search for reusable functions, patterns, and prior art before inventing (`ast-grep` / `comemory`). The best design is often "extend this thing that already works." New code is a liability you justify, not a default. When the sweep exposes something worth looking for, go look before locking the approach, because an option grounded in code that already exists beats one invented at the desk.
4. **Put three or four real approaches on the table and pick one.** Span the range — the minimal version, the conventional one, the ambitious one — rather than three variants of the same instinct; two options is a false binary unless the third is genuinely dead. Name the trade-off that actually matters for *this* problem (simplicity vs. flexibility, speed vs. correctness, blast radius vs. cleanliness), and for each option say what it forecloses, not just what it does. Take the recommended one and proceed; a menu with no pick pushes the decision back onto the user.
5. **Record the decision.** A short decision record: chosen approach, the why, the alternatives you rejected and why, the defaults you set, and the risks still open. Save anything durable (a convention, a constraint, a non-obvious call) to memory via `agent-memory` so it outlives this conversation.

**Delegate at the right tier.** Step 3's prior-art hunt is exploration — send it to a mid-tier agent (`toolu:deep-explore`) and keep the bytes out of this thread. Step 4's trade-off call is architecture: it stays with the top tier (here, or `toolu:architect`). Never let a cheap tier settle the design, and never spend the top tier on the search that feeds it. Rubric: `plugins/toolu/skills/orchestrator/references/model-routing.md`. Host mapping for delegation: [host-mapping.md](../../workflows/host-mapping.md).

## Conventions to carry forward

These are toolu defaults the later phases enforce, so decide them here rather than discovering them mid-implementation:

- **Legible structure** — one responsibility per file, files named after what they export. Code humans and AI can both navigate without a map.
- **Test strategy** — tests colocate by language: TS in a sibling `__tests__/`, Rust in a sibling `tests/`. Real-world data only; no mock-data tests. Decide *what* you'll test against now.
- **Docs** — every module and public symbol gets a concise doc line. Required, but brief — plan for it, don't bolt it on.
- **Docs in sync** — when a change touches a user-facing surface (behavior, interfaces, CLI flags, commands, config), the prose docs that describe it stay current: README, `docs/` guides, `SKILL.md` trigger text, release notes. Distinct from the code-symbol doc line above. Decide here which surfaces this task touches so the later phases require, do, and check the update.
- **Size discipline** — default ceilings of 300 code lines per TS file / 500 per Rust file (blanks and comments excluded, per-project overridable). If the design implies a giant file, split it in the design, not after the gate complains.

## What "done" looks like

A stated one-sentence intent, every design dimension either defaulted or marked N/A, a chosen approach with its trade-off named, and a decision record capturing the why and the open risks. That is the handoff to `spec` (which writes the design down) — or straight to `plan` for smaller work that doesn't warrant a written spec. Do not wait for confirmation; proceed. If you don't have those three things, you're not done brainstorming yet.
