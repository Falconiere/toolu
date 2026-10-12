# Merge queue with mechanical rebase — Plan

**Date:** 2026-10-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-12-epic-merge-queue-design.md   **Topic:** #448: the engine queues, rebases and merges gate-green PRs itself

## Evidence and approach

- Inspected: `logic.rs` (`arm_merge`, `arm_ready`, `apply_report`),
  `commit.rs` (`commit_effect`, `defer`, `close_launch` pattern),
  `effects_run.rs` (script seam for `merge`/`reconcile`), `merge_bridge.rs`,
  `github_engine.rs` (`note_babysit`, `note_change` with `BaseMoved`),
  `watch.rs`, `gate/{assess,auto,merge}.rs`, `checkpoint.rs`,
  `toolu_engine::resources::{leases::patch_lease, transitions::active_jobs}`,
  `source_apply.rs` (herdr status events and snapshots), `snapshot.rs`
  (`overlay_record`), `disk.rs` (`write_issue_fields`), `status_epics.rs`
  (`queue` projection), `verbs.rs` (`planned` lists only `queue`),
  `query.rs` (`status` over the control socket), and the test rigs
  `tests/support_test.rs`, `tests/scenario_rig_test.rs`,
  `gate/tests/gate_rig_test.rs`.
- Recalled: docs/toolu is gitignored (add with `-f`); Rust test helpers
  return `Result`; local gate env quirks (`TMPDIR=/private/tmp`).
- Approach: new focused modules (`queue.rs`, `rebase.rs`, `queue_step.rs`,
  `commit_queue.rs`) so the near-limit files (`logic.rs`, `github_engine.rs`,
  `commit.rs`, `model.rs`) only gain call sites. Every new module gets
  `tests/<module>_test.rs`. The script seam stays for tests; unset runs in
  process.

## Workstream summary

state fields and persistence → pure queue and status projection → git
mechanics → in-process head step → commit and triggers → end-to-end scenario
→ CLI verb and generated docs → Markdown docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Issue queue fields and their persistence",
    "check": "cargo test -p toolu-epic-orchestrator -- disk snapshot model logic",
    "ac_refs": ["AC-2", "AC-8"],
    "paths": ["crates/epic-orchestrator/src/model.rs", "crates/epic-orchestrator/src/disk.rs", "crates/epic-orchestrator/src/snapshot.rs", "crates/epic-orchestrator/src/logic.rs", "crates/epic-orchestrator/src/queue_fields.rs", "crates/epic-orchestrator/src/tests"],
    "input": "an issue record with queued_at and rebase_rounds written by write_issue and read back by snapshot::load; a ready report setting queued_at and a babysit report clearing it",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Pure per-repository queue and the status projection",
    "check": "cargo test -p toolu-epic-orchestrator -- queue status_epics status_api",
    "ac_refs": ["AC-2", "AC-13"],
    "depends_on": ["S1"],
    "paths": ["crates/epic-orchestrator/src/queue.rs", "crates/epic-orchestrator/src/status_epics.rs", "crates/epic-orchestrator/src/status_api.rs", "crates/epic-orchestrator/src/status_schema.rs", "crates/epic-orchestrator/src/tests"],
    "input": "a World with four ready issues in two repositories, graph unblocks 0/2/2, ready times 30/10/20 ms and two in the same millisecond",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Mechanical rebase in a fenced, snapshotted worktree",
    "check": "cargo test -p toolu-epic-orchestrator -- rebase",
    "ac_refs": ["AC-4", "AC-13"],
    "depends_on": ["S1"],
    "paths": ["crates/epic-orchestrator/src/rebase.rs", "crates/epic-orchestrator/src/rebase", "crates/epic-orchestrator/src/tests"],
    "input": "a temp bare origin with main moved ahead; a worktree on feat/x pushed to origin; a conflicting edit of the same line; an untracked file; a dirty tracked file; a resource root with the agent lease and a live job lease",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "In-process head step over the gate",
    "check": "cargo test -p toolu-epic-orchestrator -- queue_step gate",
    "ac_refs": ["AC-1", "AC-5", "AC-6", "AC-7", "AC-12"],
    "depends_on": ["S3"],
    "paths": ["crates/epic-orchestrator/src/queue_step.rs", "crates/epic-orchestrator/src/gate.rs", "crates/epic-orchestrator/src/gate", "crates/epic-orchestrator/src/tests"],
    "input": "HTTPS fixture PR states (wait, fix, merge, behind, merged, closed), a merge PUT answering 409 'Head branch was modified', a protection refusal, an arm mutation refused",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Commit the head step and drive it from reports, checks and base moves",
    "check": "cargo test -p toolu-epic-orchestrator",
    "ac_refs": ["AC-5", "AC-8", "AC-9", "AC-10"],
    "depends_on": ["S2", "S4"],
    "paths": ["crates/epic-orchestrator/src"],
    "input": "outcome documents rebased/handoff/busy/stale/fix/waiting/uncertain/disarmed committed to a World; a babysit tick returning Act for a ready issue; a BaseMoved change; herdr status events working/idle",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "End-to-end queue scenario on real git and the fixture",
    "check": "cargo test -p toolu-epic-orchestrator -- queue_scenario",
    "ac_refs": ["AC-3", "AC-4", "AC-8", "AC-9", "AC-13"],
    "depends_on": ["S5"],
    "paths": ["crates/epic-orchestrator/src"],
    "input": "Scenario rig: epic o/r#1 with children 2, 3, 4 launched into real worktrees, each pushing a branch to the local origin; GitHub fixture PR states per step; a conflicting fourth child; a base move while waiting",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "toolu epic queue verb, control op, generated CLI docs",
    "check": "cargo test -p toolu-epic-orchestrator -- verbs query && cargo test -p toolu-cli --test contract && cargo xtask gate --only docs-cli --only cli-compat",
    "ac_refs": ["AC-2", "AC-11"],
    "depends_on": ["S2"],
    "paths": ["crates/epic-orchestrator/src/verbs.rs", "crates/epic-orchestrator/src/query.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/tests", "crates/cli/tests", "docs/cli"],
    "input": "toolu epic queue with no registry (empty queue) and against a running engine with two ready issues",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "Worker brief, skill, README, AGENTS.md and OpenCode copies",
    "check": "cargo xtask check-markdown-cli && bun run check:opencode-surface",
    "ac_refs": ["AC-11"],
    "depends_on": ["S7"],
    "paths": ["plugins/epic-orchestrator", "tools/toolu-opencode/generated", "AGENTS.md"],
    "input": "the edited worker-brief.md, SKILL.md and README.md",
    "model": "inherit"
  },
  {
    "id": "S9",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8", "AC-9", "AC-10", "AC-11", "AC-12", "AC-13"],
    "depends_on": ["S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8"],
    "paths": ["."],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/epic-orchestrator/src/{queue,queue_fields,rebase,queue_step,commit_queue}.rs`
  and their `tests/<module>_test.rs`; `tests/queue_scenario_test.rs` (with a
  rig file if the scenario passes 300 code lines).
- Modify: `model.rs`, `disk.rs`, `snapshot.rs`, `logic.rs`, `commit.rs`,
  `effects_run.rs`, `github_engine.rs`, `source_apply.rs`, `merge_bridge.rs`,
  `recover.rs` (if reconcile wiring needs it), `status_epics.rs`,
  `status_api.rs`, `status_schema.rs`, `verbs.rs`, `query.rs`, `dispatch.rs`,
  `lib.rs`, `gate/merge.rs` (expose the refusal status), existing tests that
  assert `merge-ready` with the seam unset.
- Docs: `docs/cli/**`, `crates/cli/tests/snapshots/*`,
  `plugins/epic-orchestrator/skills/epic-orchestrator/{SKILL.md,references/worker-brief.md}`,
  `plugins/epic-orchestrator/README.md`, `tools/toolu-opencode/generated/**`,
  `AGENTS.md`.

## Verification

- End to end: S6 merges three PRs in queue order on real git with two engine
  rebases and zero prompts, hands a conflict to the worker with the worktree
  byte-identical to its snapshot, and rebases again after a base move.
- Failure and boundary: busy worker, live job, dirty tracked file, 409 stale
  merge, protection-only admin, timeout held, failed tick stays queued.
- Docs: `docs/cli` regenerated and gated; Markdown–CLI gate; OpenCode
  surface drift check.
- Gate: `cargo xtask gate` (through the epic job wrapper), with
  `TMPDIR=/private/tmp`.
- Delivery: scoped commits per step (pushed after their tests pass), then
  `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-12-epic-merge-queue.md --verify`,
  `toolu-review:review` with version 2 push-review state, `verdict.js status`
  reporting `overall: ready`, push, the PR against `main`
  (`Closes Falconiere/toolu#448`, `Part of Falconiere/toolu#402`), then
  `pr-babysit:babysit`.

Plan review (2026-10-12): 🟡 delivery steps missing from Verification —
fixed above. Every AC maps to a step (`AC-1`…`AC-13`); Jev alignment 0.78
for full coverage. Approved.
