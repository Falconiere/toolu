---
name: spec-review
description: "Use to review the spec or poke holes in this spec before planning or building. Validates authored contract sections, real-data acceptance evidence, scope, and blocking open questions."
---

# Spec Review

Review the spec adversarially before it becomes a plan. Judge whether a
competent builder could proceed without guessing; do not add replacement design
work or praise.

## Checklist

Validate exactly the sections authored by `spec`:

- **Problem / Non-Goals** — the user pain and boundary are concrete.
- **Architecture / Interfaces / Schema** — one decided approach, its trade-off,
  and buildable contracts/paths are present.
- **Failure modes and edge cases** — bad, empty, absent, partial, concurrent,
  and boundary behavior is stated, including failure propagation or recovery.
- **Acceptance criteria** — every `**AC-<n>:**` names an observable real-data
  outcome, rather than an implementation activity or “works correctly.”
- **Acceptance evidence** — every AC has a representative real input or
  fixture, expected observable result, applicable boundary/failure case, and a
  runnable check. No mock substitute may stand in for the data under test.
- **Documentation impact** — affected user-facing documentation is named, or a
  justified `None` is explicit.
- **Open Questions** — each has an owner and is explicitly non-blocking; an
  unanswered question that changes scope, interface, behavior, safety, or a
  required check is a blocker, not merely owned.

Also reject unstated scope expansion, missing real-data evidence, or an
architecture that cannot meet the documented size/layout constraints.

## Output

Write one finding per line, with location, severity, problem, and fix:

```
<section>: 🔴 blocker: <problem>. <fix>.
<section>: 🟡 should-fix: <gap>. <fix>.
<section>: 🔵 consider: <minor>. <fix>.
```

Stamp `**Status:** Approved` only when all authored sections and every AC's
evidence are sufficient for planning. Otherwise stamp `**Status:** Needs
changes`, list blockers, and return the spec for revision.
