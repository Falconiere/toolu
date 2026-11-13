# Detached plan-ledger run

**Date:** 2026-11-13  
**Mode:** Delivery

## Outcome

`toolu ledger run <DOC> --detach` returns a job id promptly. Its copy runs the
existing checks, records their result, and announces completion. If the working
tree changes while a check runs, the copy records `stale: true`, exits nonzero,
and leaves the prior ledger byte-identical.

## Decisions and evidence

- Keep blocking `run` as the default. Issue #876 and epic #869 specify opt-in
  detachment; `crates/toolu/src/ledger.rs` owns the argument.
- Reuse `toolu_state::tree_sha` from merged #873 and
  `toolu_engine::resources::job_results` from merged #872. The latter already
  stores `stale`, output, state, and delivery time, and announces `job-done`.
- Relaunch the native binary with a hidden result id, following
  `crates/epic-orchestrator/src/job_detach.rs`. The child calls the existing
  blocking ledger path so its check behavior and exit codes remain familiar.
- Check the captured tree before any detached ledger write. A detached
  `--step` must defer the current early `running` write; otherwise an edit
  during its check could not leave the ledger untouched. Jev ranked this
  guarded write approach above restoring an already written ledger or
  rejecting `--step`.

## Boundaries and alternatives

The result uses the bound resource root and key when there is a binding; an
ordinary checkout uses the existing `resource_home` and an empty key. This
preserves the ledger's current bound and unbound behavior and adds no second
result store, job queue, or snapshot checkout. Restoring an overwritten ledger
after detecting staleness would expose a partial write and could erase another
writer's update. Rejecting `--step` would narrow the requested `run` interface.
The stale check compares the tree at the specified boundaries; it does not
freeze the repository while checks run.

## Risks and handoff

The existing `--step` pre-write and `--json` output need explicit coverage.
The next phase is a spec with real Git tests for prompt return, success,
staleness, and unchanged blocking cases.
