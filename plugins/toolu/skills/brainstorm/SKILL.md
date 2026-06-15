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
2. **Resolve the unknowns that actually change the design.** Ask about the decisions you cannot settle from the code or a sensible default: data source, auth model, UX shape, scope boundary, failure behavior. Batch the questions; don't trickle them. Don't ask about things you can reasonably decide yourself — that wastes the user's attention on noise.
3. **Find what already exists.** Search for reusable functions, patterns, and prior art before inventing (`ast-grep` / `comemory`). The best design is often "extend this thing that already works." New code is a liability you justify, not a default.
4. **Weigh two or three approaches and recommend one.** Name the trade-off that actually matters for *this* problem — simplicity vs. flexibility, speed vs. correctness, blast radius vs. cleanliness — and say which way you'd go and why. A menu with no recommendation pushes the decision back onto the user; have an opinion.
5. **Record the decision.** A short decision record: chosen approach, the why, the alternatives you rejected and why, and the risks still open. Save anything durable (a convention, a constraint, a non-obvious call) to memory via `agent-memory` so it outlives this conversation.

## Conventions to carry forward

These are toolu defaults the later phases enforce, so decide them here rather than discovering them mid-implementation:

- **Legible structure** — one responsibility per file, files named after what they export. Code humans and AI can both navigate without a map.
- **Test strategy** — tests colocate by language: TS in a sibling `__tests__/`, Rust in a sibling `tests/`. Real-world data only; no mock-data tests. Decide *what* you'll test against now.
- **Docs** — every module and public symbol gets a concise doc line. Required, but brief — plan for it, don't bolt it on.
- **Docs in sync** — when a change touches a user-facing surface (behavior, interfaces, CLI flags, commands, config), the prose docs that describe it stay current: README, `docs/` guides, `SKILL.md` trigger text, release notes. Distinct from the code-symbol doc line above. Decide here which surfaces this task touches so the later phases require, do, and check the update.
- **Size discipline** — default ceilings of 300 code lines per TS file / 500 per Rust file (blanks and comments excluded, per-project overridable). If the design implies a giant file, split it in the design, not after the gate complains.

## What "done" looks like

A confirmed one-sentence intent, an agreed approach with its trade-off named, and a decision record capturing the why and the open risks. That is the handoff to `spec` (which writes the design down) — or straight to `plan` for smaller work that doesn't warrant a written spec. If you don't have those three things, you're not done brainstorming yet.
