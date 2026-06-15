---
name: plan
description: Use after the design is agreed (and specced, for larger work) and you need a concrete implementation plan before touching code. Produces a concise, scannable, executable plan. Native toolu workflow; the planning phase of brainstorm → spec → spec-review → plan → plan-review → execution → execution-review → test.
---

# Plan

The planning phase of the toolu workflow — after `spec-review`, before `plan-review`. Turns an agreed design into a written plan another session (or subagent) can execute without re-deriving context.

**Trigger phrases:** write a plan, plan this out, how do we implement, break this down.

## Precondition

A design exists — ideally a reviewed spec (`spec` + `spec-review` ran for larger work), or at least an agreed brainstorm decision. If intent or constraints are still fuzzy, go back to `brainstorm`; if the work is big and the contract isn't written down, go to `spec` first.

## Plan shape (keep it tight)

1. **Context** — why this change, the problem it solves, intended outcome. 2–4 sentences.
2. **Approach** — the chosen design only, not the alternatives. Name the reused functions/utilities with their paths.
3. **Steps / workstreams** — ordered, each independently verifiable. For a pattern repeated across many files, describe it once and list a few representative paths — don't enumerate every file.
   - For non-trivial work (features/refactors/behavior changes), emit the plan doc at `docs/toolu/plans/<date>-<slug>.md` with a machine-readable steps block under a heading literally `## Steps (machine-readable)` — a single fenced ` ```json ` array of `{id,title,check}`, where `check` is a runnable command (exit 0 = green). This block is the ledger contract `execution` tracks against.
4. **Critical files** — exact paths to create or modify.
5. **Verification** — how to prove it works end-to-end: the commands to run, the tests to add, the real-data path to exercise.

## Opinions to encode in every plan

- **Layout** — one responsibility per file; files named after their export; tests in `__tests__/` (TS) or `tests/` (Rust), kept flat.
- **Tests** — real-world data only, NO mocks. Name the fixtures/real inputs.
- **Docs** — a concise doc line per new module/public symbol is part of "done", not a follow-up.
- **Docs in sync** — when a step changes a user-facing surface (public API, CLI flags, commands, config, documented behavior), emit an explicit doc-update step touching the affected README / `docs/` guides / `SKILL.md` triggers / release notes; for ledger-tracked plans give it a runnable `check`.
- **Size** — respect the per-project line limits (default 300 TS / 500 Rust); if a step grows a file past them, the plan must split it.
- **Gate-aware** — the quality gate blocks further edits while failing; sequence steps so each lands clean.

## Output

A plan file that is scannable in a minute and executable without guesswork. Hand off to `plan-review` to pressure-test it, then to `execution`.
