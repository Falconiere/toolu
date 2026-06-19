---
name: plan-review
description: "Use to review an implementation plan BEFORE writing code — check it's executable, complete, and won't collapse on contact with the codebase. Tells: \"review the plan\", \"is this plan ready\", \"poke holes in the plan\", \"did we miss a step\". Pairs with the `plan` skill; runs between plan and execution in the toolu workflow."
---

# Plan Review

A plan that looks fine on paper but skips a dependency, reuses nothing, or has unverifiable steps turns into thrash during execution. This phase pressure-tests the plan before anyone writes code. It is adversarial on purpose — find the gap now (a sentence to fix) rather than mid-implementation (a half-built branch to unwind).

Read the plan and run it against the checklist. For each item ask *would an implementer be blocked, misled, or forced to guess?* — that's the bar.

## Checklist

- **Traces to the spec** — does the plan implement what the (reviewed) spec/design actually decided, with nothing silently added or dropped? Scope drift starts here.
- **Steps are independently verifiable** — can each step be proven done on its own (a command to run, a test to pass), or are there "and then it works" leaps? A step you can't verify is a step you can't trust.
- **Ledger steps are runnable** — for non-trivial work, assert **every step has a** concrete, runnable `check` command in the `## Steps (machine-readable)` block (else blocker), and reject **empty steps** (`steps:[]` or a step missing `id`/`title`/`check`). The checker can only stamp green what it can run.
- **AC refs resolve** — every step `ac_refs` id names a real `**AC-<n>:**` in the plan's declared spec. Run `bash plugins/toolu/hooks/lib/plan-ledger.sh` (via `pl_check_ac_refs <plan_doc> <spec_doc>`); a dangling `ac_ref` (no such AC in the spec) is a blocker. Spec-less plans (`**Spec:** none`) skip this — no AC ids to resolve against.
- **Reuses what exists** — does it call the helpers/utilities already in the codebase (named, with paths), or reinvent them? New code that duplicates old code is a finding.
- **Critical files named** — are the files to create/modify listed concretely, not "update the relevant modules"? Vagueness becomes guesswork in execution.
- **Error handling is planned, not deferred** — does the plan say how failures are handled (propagate/match/convert), or leave it as "add error handling later"? Later means never; the gate will block it anyway.
- **Tests are real-data and located** — does each behavior have a planned test against real inputs (no mocks), in the right place (`__tests__/` / `tests/`)?
- **Respects the conventions** — file size ceilings, one-responsibility files named after their export, concise docs. If a step grows a giant file, the split must be in the plan.
- **Docs in sync** — if the plan changes behavior, interfaces, CLI flags, commands, or config but has no step updating the user-facing docs (README, `docs/` guides, `SKILL.md` triggers, release notes), that's a finding.
- **Gate-aware sequencing** — are steps ordered so each lands clean? The quality gate blocks further edits while failing; a plan that stacks changes before a green checkpoint will stall.
- **Verification is end-to-end** — is there a section proving the whole thing works (run the code / MCP / tests), not just unit checks?

## Output

One line per finding, severity-tagged, location + problem + fix — no praise, no restating what's fine:

```
<step/section>: 🔴 blocker: <what's missing/wrong>. <the fix>.
<step/section>: 🟡 should-fix: <gap>. <the fix>.
<step/section>: 🔵 consider: <minor>. <the fix>.
```

End with a verdict, and **stamp the plan's `**Status:**` header field** to match (mirroring how `spec-review` stamps the spec) — `execution`'s preflight gate reads it:
- **Approved** → stamp `**Status:** Approved`; ready for `execution`.
- **Needs changes** → stamp `**Status:** Needs changes`; list the blockers that must close first; back to `plan`.

Flag only what blocks confident execution. A plan that reads clean but can't be verified step-by-step is not clean — say so.
