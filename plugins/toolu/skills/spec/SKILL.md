---
name: spec
description: "Use AFTER a design is agreed (brainstorm done) and BEFORE planning, when the work deserves a written contract — a new system, a cross-cutting feature, anything multiple sessions build against. Tells: \"write the spec\", \"spec this out\", \"document the design\", \"pin down the requirements\". Produces a concise design spec (problem, non-goals, architecture, interfaces, acceptance criteria) under docs/toolu/specs/. Spec phase of the toolu workflow. Skip for mechanical or already-specced work."
---

# Spec

Second phase of the toolu workflow. A spec is the contract: it says what we're building and how it must behave, precisely enough that `plan` can turn it into steps and `test` can turn it into checks — without re-litigating the design. Brainstorm decided the *shape*; the spec writes it down so it stops living in one head.

Write a spec when the cost of being wrong is high: a new system, a public interface, a cross-cutting change, anything more than one session will build against. Skip it for mechanical work or anything already specced — a spec for a rename is ceremony, not value.

## Precondition

A design is agreed (`brainstorm` ran, or the user supplied clear requirements). If the shape is still fuzzy — competing approaches, unknown constraints — go back to `brainstorm`. A spec written over guesses just launders them into something that looks decided.

## Format

Write to `docs/toolu/specs/<YYYY-MM-DD>-<slug>-design.md` (today's date from the environment; kebab-case slug from the title). Use this template — keep every section tight, cut anything that isn't load-bearing:

```markdown
# <Title> — Design

**Date:** <YYYY-MM-DD>   **Status:** Draft   **Author:** <name>   **Topic:** <one line>

## Problem
2–4 sentences: the user pain, why it matters now. Not the solution.

## Non-Goals
Numbered, explicit out-of-scope. The boundary is half the value of a spec —
it's what stops scope creep in `plan` and `execution`.

## Architecture
The chosen approach. Name the one trade-off that actually drove the decision
(simplicity vs flexibility, blast radius vs cleanliness). Reference existing
code/utilities to reuse, with paths.

## Interfaces / Schema
Concrete signatures, types, JSON shapes, file paths, config keys — enough that
someone could start building without guessing the contract.

## Acceptance criteria
Testable outcomes phrased against real-world data (no mocks). Each one should
map to a check `test` can write. "X produces Y given real input Z", not "X works".

## Open Questions
Unresolved decisions with an owner. An honest spec names what it doesn't know.
```

## Why these sections

- **Problem before solution** — if you can't state the pain in a few sentences, the solution is aimed at nothing.
- **Non-goals** — the cheapest scope control there is; an unwritten boundary gets crossed.
- **Architecture with the trade-off named** — a decision without its *why* gets re-argued the moment it's inconvenient.
- **Concrete interfaces** — vagueness here becomes rework in `execution`; pin the contract now.
- **Real-data acceptance criteria** — they are the bridge to `test`; criteria you can't test on real inputs aren't criteria.

## Conventions carried forward

Decide here so the later phases inherit, not discover: legible structure (one responsibility per file, named after its export), real-world-data testing, concise-but-required docs, **Docs in sync** (when the change touches a user-facing surface — behavior, interfaces, CLI, commands, config — the prose docs that describe it are updated in the same change: README, `docs/` guides, `SKILL.md` triggers, release notes; name it in Acceptance criteria), and the per-project size ceilings — if the architecture implies a giant file, split it in the spec.

## What "done" looks like

A spec file with every section filled, `Status: Draft`, no hand-waving in Architecture or Acceptance criteria, and Open Questions either answered or owned. Hand off to `spec-review` to pressure-test it, then to `plan`.
