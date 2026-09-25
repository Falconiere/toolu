---
description: Use to implement and deliver a repository task end to end. Runs brainstorm, approved spec, approved ledger plan, real-data execution, PR, and babysit on Claude Code or Codex.
name: delivery-flow--delivery-flow
---

# Delivery flow

Invoking this skill authorizes commit, push, PR creation, and the `pr-babysit:babysit` handoff once the checks below pass. Run the complete sequence for every task, including small fixes: brainstorm → spec → spec review → plan → plan review → execution with real-data tests → PR → `pr-babysit:babysit`. This skill is the only public entry point; the phase files in `references/` are private procedures. Read each one when entering its phase. Use the active host's `/delivery-flow:delivery-flow` or `$delivery-flow:delivery-flow` invocation as appropriate.

Before running ledger or verdict commands, locate the enabled, installed
`toolu@toolu` plugin and set `TOOLU_LIB` to its `hooks/lib` directory. Claude
Code exposes `installPath` in `claude plugin list --json`; Codex exposes
`source.path` in `codex plugin list --json`. Prefer an enabled project install
for the current repository, then an enabled user install. Confirm
`$TOOLU_LIB/plan-ledger.sh` and `$TOOLU_LIB/verdict.sh` exist. In a toolu source
checkout, `plugins/toolu/hooks/lib` is also valid. The task repository need
not contain toolu's source tree.

## Sequence

1. **Brainstorm:** Read [brainstorm.md](references/brainstorm.md) and record the outcome, repository evidence, risks, and decisions. Use its compact path for bounded work, but never skip the phase. Resolve material uncertainty before drafting the spec.
2. **Spec:** Read [spec.md](references/spec.md). Write the design with observable acceptance criteria, real-input evidence, failure behavior, and documentation impact. A small fix still gets a concise spec.
3. **Spec review:** Read [spec-review.md](references/spec-review.md). Review against the authored contract. If `Status: Needs changes`, fix the findings and repeat this phase until `Status: Approved`. Do not plan against a rejected spec.
4. **Plan:** Read [plan.md](references/plan.md) and [ledger.md](references/ledger.md). Write a machine-readable plan with runnable checks, paths, dependencies, and AC references. A small fix still gets a compact ledger plan.
5. **Plan review:** Read [plan-review.md](references/plan-review.md). Correct and repeat until `Status: Approved`. Do not execute a rejected plan.
6. **Execution:** Read [execution.md](references/execution.md) and use [test.md](references/test.md) during each behavior step. Run `bash "$TOOLU_LIB/plan-ledger.sh" preflight` before editing. Produce real-data red → green evidence, stamp each step, keep docs synchronized, and run the full quality gate. A failed check stops progress; fix it and resume from that phase. No new runtime state gate is needed.
7. **Delivery:** The execution reference owns the exact order: scoped commit; final ledger `run <plan_doc> --verify`; `toolu-review:review` with complete version: 2 state; `bash "$TOOLU_LIB/verdict.sh" status` reporting `overall: ready`; push; locate or create and verify the default-branch PR; invoke `pr-babysit:babysit` with no arguments.

## Blockers and resume

Before any delivery write, require a successful authenticated `gh api user` call, a non-default branch, the installed `toolu`, `toolu-review`, and `pr-babysit` skills, and every review and quality check above. Name the exact missing prerequisite and stop; invocation already supplies delivery authorization, so do not ask for it again. On a review rejection or failed check, preserve completed evidence, repair the finding, and resume from that phase. Recheck any downstream evidence made stale by the repair. Never claim PR or babysit completion without observing it.
