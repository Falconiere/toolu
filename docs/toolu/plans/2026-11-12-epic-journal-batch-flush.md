# Epic journal batch flush — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-epic-journal-batch-flush-design.md   **Topic:** One JSONL sync per consecutive outbox journal batch.

## Evidence and approach

Issue #874 (blocked by merged #870 / PR #889) requires consecutive front-of-outbox journal steps to share one lock and one JSONL flush, with every row durable before follow-ups. Current `Engine::flush` pops one step (`server.rs`) and `apply_journal` calls `journal::append` per row. #870 left a single `flush` helper with a test counter on the day file only. Jev chose engine drain + `append_batch` with one end-of-batch `sync_all`.

Concrete shape:
1. `write::write_lines` opens the day file once, appends every line, calls `flush` once, returns per-row `Placed` values; torn-tail repair stays on the first open.
2. `index::append_batch_locked` assigns consecutive seqs, writes via `write_lines`, publishes each row's cache update; empty slice is a no-op.
3. `journal::append_batch` takes the lock once around that path (same wait as `append`).
4. `Engine::flush` drains the journal prefix, stamps, `append_batch`, then for each stored row runs `commit_journal` follow-ups through `apply`; a test-only phase sink records `flushed` after sync and `follow` before each row's follow-up applies. Non-journal steps keep single `apply`.

## Workstream summary

Add failing batch flush and follow-up-order tests, implement `append_batch` and engine drain, update journal docs, then run scoped and full checks.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Batch append shares one flush; engine follow-ups run after it",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- journal:: server::",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index/write.rs", "crates/epic-orchestrator/src/journal/index/tests", "crates/epic-orchestrator/src/server.rs", "crates/epic-orchestrator/src/tests/server_test.rs"],
    "input": "real temporary journal dir: append_batch of three records yields consecutive sequences and one flush; Engine outbox with three journals then two launch-done rows with pending issues records flushed then follow phases"
  },
  {
    "id": "s2",
    "title": "Recover target and documentation stay green",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- recover:: && env -u PUSH_REVIEW_BASE cargo xtask check-changed --base origin/main && env -u PUSH_REVIEW_BASE cargo xtask check-markdown-cli",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates/epic-orchestrator/src", "docs/epic-orchestrator/README.md", "docs/toolu/specs/2026-11-12-epic-journal-batch-flush-design.md", "docs/toolu/plans/2026-11-12-epic-journal-batch-flush.md"],
    "input": "existing recover:: suite plus branch diff docs for one-flush-per-batch wording"
  },
  {
    "id": "s3",
    "title": "Run the complete quality gate",
    "check": "env -u PUSH_REVIEW_BASE cargo xtask gate",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "the committed branch against current origin/main"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal.rs`, `journal/index.rs`, `journal/index/write.rs` and sibling tests
- `crates/epic-orchestrator/src/server.rs` and `tests/server_test.rs`
- `docs/epic-orchestrator/README.md`
- This plan and its approved design spec

## Verification

`journal::` / `server::` prove batch flush count, sequences, and follow-up order on real temp dirs. `recover::` plus `check-changed` / markdown CLI cover regressions and docs. Full `cargo xtask gate` once before push. No CLI regeneration. Delivery: scoped commit, `toolu ledger run <plan> --verify`, code-review version 2 ready, verdict ready, PR with Closes #874 and Part of #869, then `report babysit`.
