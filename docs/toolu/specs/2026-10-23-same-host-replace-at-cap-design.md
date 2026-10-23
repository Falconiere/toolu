# Same-host launch --replace at the agent cap — Design

**Date:** 2026-10-23   **Status:** Draft   **Author:** Falconiere Barbosa   **Topic:** pin that a same-host replace keeps its own lease and never needs a second slot

## Problem
Issue #587 reports `toolu epic launch --replace` on the same host failing with `claude capacity exhausted` at the agent cap. The report attributes it to migrations reserving the target before releasing the source.

## Non-Goals
1. Reclaiming dead leases (#583, merged).
2. Changing capacity rules for cross-host moves.

## Architecture
Verified on `origin/main` (43d1f08e): `reserve` in `crates/core/engine/src/resources/transitions.rs` counts only leases whose token differs from the moving one, and `launch_reservation` reuses the issue's saved lease token for a replace. A same-host replace therefore already keeps its own slot and stops the old agent under the same token. The refusal in the report came from ghost leases of dead agents, which occupied the cap itself (#583, fixed by `reconcile_agent_leases`). The remaining gap is the absence of a regression test, so this change adds one.

## Interfaces / Schema
No interface change. Test: `launch::admission::tests::a_same_host_replacement_at_the_agent_cap_keeps_its_own_slot`.

## Failure modes and edge cases
- Another live lease fills the cap: the replace still refuses with the capacity error (unchanged).
- Lease token differs from the record: refuses with "existing launch ownership differs" (unchanged).

## Acceptance criteria
- **AC-1:** With `maxAgents` 1 and one running Claude lease holding a live pane, `launch_reservation` for Claude with `replace` returns the same owner token and leaves exactly one lease.

## Acceptance evidence
AC-1: real resource home and herdr fixture; `cargo test -p toolu-epic-orchestrator --lib same_host_replacement`.

## Documentation impact
None — no behaviour or interface changes.

## Open Questions
None. Decision: no production change, because the verified code already satisfies the expected behaviour; the regression test locks it in.
