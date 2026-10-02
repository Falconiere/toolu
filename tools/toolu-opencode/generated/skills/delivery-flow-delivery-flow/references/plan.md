---
name: plan
description: "Private delivery-flow plan phase. Write a ledger-backed plan for every task after spec approval."
---

# Plan

**Evidence first:** recall relevant decisions, inspect the declared spec, and
search the codebase for existing paths, helpers, tests, and docs before drafting.
Do not turn assumptions into steps.

Use the mandatory [Jev workflow](semantic-judgments.md) for
classifications that change step handling. Derive paths, dependencies, and ordering from repository evidence.

When a ledger step needs delegation, declare its model tier using the
[`model-routing` rubric](model-routing.md) and map
host-specific delegation through [host-mapping.md](host-mapping.md).

## Choose the planning depth

**Mechanical work** — a typo, rename, formatting-only change, or dependency
bump with no behavior change — gets a compact ledger plan with direct
verification when invoked through delivery-flow.

Behavior changes, features, fixes, refactors, and changes with meaningful risk
always receive a compact plan at `docs/toolu/plans/<YYYY-MM-DD>-<slug>.md`.
Use the reviewed spec produced earlier in delivery-flow.

## Plan shape

1. **Header** — `**Date:** <YYYY-MM-DD>   **Status:** Draft   **Spec:**
   <path | none>   **Topic:** <one line>`.
2. **Evidence and approach** — brief outcome, constraints, reused paths, and
   chosen design. Cite what was recalled or inspected.
3. **Workstream summary** — a short, non-duplicative overview of the sequence
   (for example, “parser → validation → docs”). It is not a second task list.
4. **Steps (machine-readable)** — one JSON array under the literal heading
   `## Steps (machine-readable)`. This ledger is the **sole detailed step list**;
   use the shared reference for its schema and real-input evidence conventions.
5. **Critical files** — exact paths to create or modify.
6. **Verification** — end-to-end outcome, real inputs, failure/boundary checks,
   and required documentation synchronization.

Use the canonical ledger shape and optional-field/path-freshness guidance in
[the shared ledger reference](ledger.md); do not duplicate it in a
plan. Keep checks runnable and order dependent work explicitly.

Every user-facing behavior, interface, CLI, command, or config change needs an
explicit documentation step and check. Keep files focused, use colocated tests,
and include the real inputs and failure propagation that prove the behavior.

## Handoff

Leave the plan `Draft` and send it to `plan-review`. Only an approved plan moves
to `execution`.
