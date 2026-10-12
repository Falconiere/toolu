# Brainstorm: merge queue with mechanical rebase (#448)

Delivery mode, Full path: the engine starts merging unattended, adds
persisted queue state, and pushes to worker branches.

## Capsule

- **Outcome:** the resident engine (`toolu epic engine`) keeps one merge queue
  per repository. Ready issues (worker `ready`, or a green babysit tick) enter
  it, ordered by the graph's `unblocks` (descending), then the time they became
  ready, then key. Only the head is worked: the engine gates it, rebases it
  itself with plain git when it is behind, arms auto-merge pinned to its head
  while checks run, and merges it pinned to the verified SHA when the gate says
  `merge`. A conflict aborts the rebase and sends the worker `REBASE`; a
  failing check, an unresolved thread or a babysit finding returns the issue to
  babysit. `toolu epic queue` prints the queue; `toolu epic gate` stays the
  manual gate.
- **Material defaults:**
  - Queue membership is derived from engine state (phase `ready`, stage
    `running`/`awaiting_merge`, a PR, a repository). The record persists
    `queued_at` so the order survives a restart; no separate queue file.
  - The head step is the existing `merge` action. With `TOOLU_EPIC_SCRIPTS`
    unset it runs in process (gate, rebase, auto-merge, merge) instead of
    returning `deferred`; the script seam stays as the test seam for the
    fault-injection suites.
  - Fence: `patch_lease` sets the agent lease stage to `rebasing` (job
    admission refuses any stage but `starting`/`running`), then the engine
    checks no managed job runs in the worktree, rebases, and patches the stage
    back to `running`. No change to `toolu-engine`.
  - Parked means phase `ready` and the last herdr status of the agent is not
    `working`. A worker that is not parked is skipped; the event is journaled
    and the head waits for the next check.
  - A dirty worktree, a detached or mismatched branch, or a conflict hands the
    rebase to the worker (`REBASE`), never `--autostash`.
  - A merge refused because the head moved (HTTP 409 or 422 "head") is not a
    failure: the head is gated again at the next check.
  - Journal: `rebase` records with `{"by":"engine"|"worker","round":n}` per
    issue.
- **Non-goals:** stacked merge trains, rewriting the orchestrator skill's
  engine section (#452), removing the script seam, a write API over HTTP.
- **Repository evidence:**
  - `crates/epic-orchestrator/src/gate/*` already ports `merge-gate.ts`
    (#516); `run_gate` merges pinned to the head, `--admin` only after a
    protection-only refusal, and disarms auto-merge on `rebase`/`fix`.
  - `logic::arm_merge` arms the `merge` action on `ready`; `commit::defer`
    turns it into a `merge-ready` judgment (`merge_bridge.rs`).
  - `checkpoint::snapshot` writes `refs/epic-wip/<key>`.
  - `github_engine::note_change` sees `BaseMoved` and `PrMerged`;
    `watch::after_merge` requests immediate checks of the repository.
  - `graph::GraphIssue.unblocks` is the transitive downstream count.
- **Risk:**
  - An unattended merge on a wrong SHA. Mitigated by pinning both the
    auto-merge (`expectedHeadOid`) and the REST merge (`sha`), disarming
    before every engine push, and gating again after any refusal.
  - Touching an active worker. Mitigated by the parked check plus the lease
    fence, and by aborting on any git failure.
  - GraphQL cost: one gate per queue head per 3-minute check, not per queued
    PR.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Persistence | Derived queue, `queued_at` in `issues/<key>.json` | Status API already projects ready issues; records already carry engine fields (`write_issue_fields`) | Order of two issues readied in the same millisecond falls back to key |
| Interface | `toolu epic queue [--json] [--repo R]`; `planned` verb removed | `verbs.rs` lists only `queue` as planned (#448) | `check-cli-compat` exempts `planned` |
| Security | Push only `HEAD:<pr head ref>` with `--force-with-lease=<ref>:<old sha>`; refuse when the head ref equals the base | Authorization boundary in epic #402 | A misconfigured remote; push errors abort and hand off |
| Cost | One gate per head per check | #447 fixed 3-minute schedule | None new |
| Reversibility | Snapshot before every rebase; abort restores | `checkpoint.rs` | Snapshot failure stops the rebase |

## Alternatives

| Decision | Chosen | Rejected and why |
|---|---|---|
| Queue store | Derived from issue state plus `queued_at` (Jev 0.80) | `queue.json` per repository (0.20): a second source of truth to keep in sync with phases |
| Fence | `patch_lease` to `rebasing`, then back (Jev 1.00) | A new core fence pair (0.00): a core change for what `patch_lease` already does. Launch lock only (0.00): does not stop managed jobs |
| Dirty worktree | Hand to the worker (Jev 0.73) | `--autostash` (0.24): rewrites the worker's uncommitted state. Silent retry (0.03): never progresses |
| Script seam | Keep it for tests; run in process when unset (Jev 1.00) | Remove it now (0.00): rewrites every fault suite inside an already large change |
| Prompting each worker to rebase | Rejected by the epic | One LLM round per merge elsewhere |
| Merge without being up to date | Rejected by the epic | Breaks the merge invariant |
