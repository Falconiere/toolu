---
name: plan-review
description: "Use to review the plan or poke holes in the plan before execution. Verifies ledger steps, AC coverage, real-input checks, paths, dependency order, docs, and PR-delivery readiness."
---

# Plan Review

Pressure-test a draft plan before code changes begin. Find the gap that would
force an implementer to guess; do not restate the plan or praise it.

## Checklist

- **Evidence and scope** — approach follows the approved spec or agreed
  requirements, reuses discovered paths, and adds no silent scope.
- **AC-to-step coverage** — every spec AC has at least one ledger `ac_refs`
  mapping. Run `pl_check_ac_refs <plan_doc> <spec_doc>` to reject dangling refs;
  a spec-less plan must explicitly justify why no AC mapping applies.
- **Runnable evidence** — every ledger step has a non-empty runnable `check`;
  each behavior check identifies a real input/fixture and observable result,
  including applicable invalid or boundary input. Reject empty steps and
  happy-path-only verification.
- **Path declarations** — `paths` include every file the check reads, especially
  production and test files; omitted or stale paths invalidate freshness claims.
- **Dependency order** — `depends_on` and workstream order make prerequisites
  green before dependent work; no circular or impossible ordering.
- **Critical files and failure behavior** — all edited paths are declared and
  failure propagation/recovery is planned instead of deferred.
- **Documentation** — each user-facing behavior/interface/CLI/command/config
  change has an explicit documentation step with a runnable check.
- **PR-delivery readiness** — plan includes end-to-end real-data verification,
  the final ledger verification, scoped commit/push expectations, and the
  delivery prerequisite for the automatic PR/babysit handoff.

## Output

Use one finding per line:

```
<step/section>: 🔴 blocker: <problem>. <fix>.
<step/section>: 🟡 should-fix: <gap>. <fix>.
<step/section>: 🔵 consider: <minor>. <fix>.
```

Stamp `**Status:** Approved` only when the plan is executable and delivery-ready;
otherwise stamp `**Status:** Needs changes` and return it to `plan`.
