---
name: execution
description: Use when you have a reviewed plan to implement. Drives the plan step by step with verification checkpoints, respects the quality gate, and delegates heavy work to subagents to keep context compact. Native toolu workflow; the execution phase of brainstorm → spec → spec-review → plan → plan-review → execution → execution-review → test.
---

# Execution

The execution phase of the toolu workflow — it comes after `plan-review` and hands off to `execution-review`. Carries out a reviewed plan with discipline: small steps, evidence before claims, never skip the gate.

**Trigger phrases:** execute the plan, implement this, start building, work through the plan.

## Precondition

A reviewed plan exists (`plan` + `plan-review` ran for non-trivial work). If there's no plan, write one first; if the plan hasn't been pressure-tested, run `plan-review` before sinking time into code built on a shaky plan.

## Loop (per step)

For ledger-tracked work, **before the first step** run `bash plugins/toolu/hooks/lib/plan-ledger.sh preflight` — it refuses to start unless the plan is `Approved` and its declared spec (if any) is `Approved`. Then read progress with `bash plugins/toolu/hooks/lib/plan-ledger.sh status` to find the next non-fresh-green step, do the loop below for it, then record it with `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc> --step <id>` — the engine requires the plan-doc positional arg, and stamps green from mechanical truth, you cannot claim it. On plan deviation, edit the steps block and note it under `## Deviations`, then re-run. Re-run any stale step (a `green` step whose diff has since changed) before calling the plan done. Before push, do a final full `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc>` so every step is fresh-green against the final code. `status` also prints an AC-coverage report (report-only): read it to confirm every spec `AC-<n>` is covered by a fresh-green step — an uncovered AC is surfaced, not yet a push blocker, but it means the goal isn't proven done.

1. **Take one step** from the plan — the smallest shippable unit.
2. **Write tests with the code** (see `test`) — real data, colocated. For a bugfix, reproduce first.
3. **Handle errors in code, never suppress them.** Every fallible call gets a real handler — propagate (`?`, rethrow), match, or convert; never swallow, never silence with a disable comment (`@ts-ignore`, `eslint-disable`, `#[allow]`). The gate enforces this on every edit; write it right the first time.
4. **Land it clean.** A PostToolUse quality gate runs on every TS/Rust edit. If it reports a violation the gate goes **failing** and blocks further edits until fixed — fix immediately; do not pile on more changes.
5. **Verify, don't assume.** Run the command, read the output. "Done" requires evidence (test pass, log, runtime check), never a guess.
6. **Checkpoint** with the user at meaningful boundaries, or after each independent workstream.

## Rules

- **Global gate** — do NOT move to the next step while any error/warning/test failure stands, even in unrelated files.
- **Delegate to stay compact** — push exploration, large reads, and parallelizable work to subagents; keep the main context lean.
- **Delegate at the step's tier** — `status` prints `model=<alias>` for the next step when the plan declared one; hand that step to a subagent on that model (`toolu:quick-task` / `toolu:implementer` / `toolu:architect`, or an explicit `model:`). No declared tier, or a step that turns out harder than planned? Fall back to the rubric and escalate one tier rather than retrying the same tier: `plugins/toolu/skills/orchestrator/references/model-routing.md`.
- **No scope creep** — do only what the plan calls for. New needs go back to `plan` (and `plan-review`), not into this step.
- **Honor the layout** — files named after their export, one responsibility each, under the line limit, docs present and concise.
- **Docs in sync** — when a step changes a user-facing surface (behavior, interfaces, CLI flags, commands, config), update the prose docs that describe it (README, `docs/` guides, `SKILL.md` triggers, release notes) in the same step; it's part of "done", not a follow-up.
- **Same approach failed twice? Stop.** Change the hypothesis (`systematic-debugging`), don't retry harder.

## What "done" looks like

Working, verified increments that match the plan, with real error handling and real-data tests, landed under a green gate. Hand off to `execution-review` to confirm the work matches the plan and the conventions hold, then to `test` for the final pass.
