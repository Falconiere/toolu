---
name: execution
description: Use when you have a reviewed plan to implement and, when delivery is authorized, carry verified changes through a PR handoff. Drives the plan step by step with evidence checkpoints and respects the quality gate. Native toolu workflow: spec → spec-review → plan → plan-review → execution → pr-babysit.
---

# Execution

The execution phase comes after `plan-review`. It carries out a reviewed plan with discipline: small steps, evidence before claims, and no skipped gate. `brainstorm` can be useful upstream when the shape is not settled; `test` is the execution-time method for producing high-signal evidence, not a final workflow phase.

**Trigger phrases:** execute the plan, implement this, start building, work through the plan.

## Precondition

A reviewed plan exists (`plan` + `plan-review` ran for non-trivial work). If there's no plan, write one first; if the plan hasn't been pressure-tested, run `plan-review` before sinking time into code built on a shaky plan.

## Loop (per step)

For ledger-tracked work, **before the first step** run `bash plugins/toolu/hooks/lib/plan-ledger.sh preflight` — it refuses to start unless the plan is `Approved` and its declared spec (if any) is `Approved`. Then read progress with `bash plugins/toolu/hooks/lib/plan-ledger.sh status` to find the next non-fresh-green step, do the loop below for it, then record it with `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc> --step <id>` — the engine requires the plan-doc positional arg, and stamps green from mechanical truth, you cannot claim it. On plan deviation, edit the steps block and note it under `## Deviations`, then re-run. Re-run any stale step (a `green` step whose diff has since changed) before calling the plan done. `status` also prints an AC-coverage report (report-only): read it to confirm every spec `AC-<n>` is covered by a fresh-green step — an uncovered AC is surfaced, not yet a push blocker, but it means the goal isn't proven done.

1. **Take one step** from the plan — the smallest shippable unit.
2. **Produce per-step real-data evidence** (use `test`) — map the relevant AC or risk to a representative real input/fixture, observable result, applicable boundary/failure case, and runner command. For a bugfix, reproduce first; record the passing output before the ledger step is stamped green.
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

Working, verified increments that match the plan, with real error handling and per-step real-data evidence, landed under a green gate. Execution owns the final local review; do not hand work to a separate review or terminal-test phase.

## Authorized delivery preflight

Only perform delivery when the user's request includes delivery authorization.
Before committing, pushing, creating a PR, or starting a durable babysitting
goal, check every prerequisite and stop before delivery with the exact unmet
prerequisite if any applies:

- delivery authorization is absent;
- GitHub auth is unavailable (`gh auth status` fails);
- the current branch is the repository default branch, detached, or otherwise not a non-default branch;
- the optional `pr-babysit` plugin is not installed and its `$pr-babysit:babysit` skill is unavailable.

When the per-step direct checks, documentation work, and prerequisites pass,
commit the scoped changes using the repository's conventions. Do not include
unrelated work.

## Local release-readiness audit

After that scoped commit and before pushing, establish all of the following
against the committed branch diff with command output, not assertion:

1. Re-run each affected step's real-data runner and ensure its AC/risk evidence is current. Read `plan-ledger.sh status` and resolve every missing or stale AC coverage entry.
2. Run `bash plugins/toolu/hooks/lib/plan-ledger.sh run <plan_doc> --verify`. This is the supported branch-wide verification command: it validates every step against the final diff and stamps the ledger only when all steps are fresh-green.
3. Confirm user-facing documentation is synchronized for every changed behavior, interface, CLI, command, or configuration surface. Treat a missing applicable doc update as a blocker.
4. Run `$toolu-review:review` (or the host's installed `toolu-review:review` invocation) against the committed branch diff. Its resulting push-review state must be v2 (`version: 2`) and cover every changed file; open findings or stale/incomplete coverage are blockers.
5. Run `bash plugins/toolu/hooks/lib/verdict.sh status`. Advance only when it reports `overall: green`; quality, plan, review, and docs must each be green.

## PR delivery

When the committed-diff audit and every prerequisite pass:

1. Push the non-default feature branch to its configured remote.
2. Discover the repository default branch, then locate or create a pull request for the current branch targeting that repository default branch. Verify that PR's number and head/base branches.
3. Invoke `$pr-babysit:babysit` with no arguments. The verified execution handoff plus this delivery authorization is sufficient authorization for its durable PR-clearing goal; do not add a handoff argument or weaken its isolated-worktree, strict-clearance, or durable-goal rules.
