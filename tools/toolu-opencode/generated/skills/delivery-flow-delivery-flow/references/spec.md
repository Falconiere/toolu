---
name: spec
description: "Private delivery-flow spec phase. Write a concise, reviewable contract for every task after brainstorm."
---

# Spec

Write a spec on every delivery-flow invocation. It is the durable contract for
`plan`, `execution`, and `test`, not a transcript of brainstorming. Start from
the brainstorm decision. Return to `brainstorm` only when
a material design choice remains undecided.

Use the mandatory [Jev workflow](semantic-judgments.md) for
requirement ambiguity and observable outcomes. Agent owns the contract and feasibility.

Write `docs/toolu/specs/<YYYY-MM-DD>-<slug>-design.md`. Keep every section
short and concrete:

```markdown
# <Title> — Design

**Date:** <YYYY-MM-DD>   **Status:** Draft   **Author:** <name>   **Topic:** <one line>

## Problem
The user pain and why it matters now.

## Non-Goals
Explicit, numbered scope boundaries.

## Architecture
Chosen approach, the decisive trade-off, and existing paths/utilities to reuse.

## Interfaces / Schema
Concrete signatures, types, JSON shapes, paths, or config keys.

## Failure modes and edge cases
Bad, empty, absent, partial, concurrent, or otherwise boundary inputs; state
the observable behavior and whether failure propagates, is converted, or is
recovered.

## Acceptance criteria
- **AC-1:** A testable outcome stated against a real input and observable result.

## Acceptance evidence
For every AC, name the representative real input or fixture, expected observable
result, applicable boundary/failure case, and runnable check that will prove it.

## Documentation impact
State the affected README, `docs/`, `SKILL.md`, command/config, or release-note
surface; say `None — <reason>` only when no user-facing documentation changes.

## Open Questions
Unresolved decisions, owner, and whether each question blocks implementation.
```

Acceptance-criterion ids use stable bold prefixes (`**AC-<n>:**`). They are
labels rather than indices: gaps are fine, but never reuse an id. Each criterion
must describe an observable real-data outcome — “given input X, produces Y” —
so a ledger step can cite it through `ac_refs` and `test` can demonstrate it.

## What done means

Every authored section is filled; acceptance evidence covers every AC; failure
behavior and documentation impact are explicit; and each open question is
resolved, owned, or clearly non-blocking. Keep `Status: Draft` and hand off to
`spec-review`.
