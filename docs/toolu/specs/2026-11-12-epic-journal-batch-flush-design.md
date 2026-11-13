# Epic journal batch flush — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Cursor   **Topic:** One JSONL sync for a consecutive outbox journal batch.

## Problem

`Engine::flush` applies journal steps one at a time, and each `append` syncs the daily JSONL file. A burst of consecutive journal rows therefore pays one `sync_all` per row even though the intent-before-effect rule only needs every row of that burst durable before any follow-up runs.

## Non-Goals

1. A writer thread or cross-process co-batching.
2. Changing sequence allocation, lock wait policy, or cache rebuild rules from #870.
3. Reordering non-journal outbox steps relative to journals.

## Architecture

Drain the longest prefix of `Step::Journal` at the front of the outbox under the existing flush loop. Stamp each record, call one `journal::append_batch` (one lock hold, consecutive sequences, N line writes, one JSONL `sync_all`, then cache publish per row), then run each stored row's `commit_journal` follow-ups in order through the existing `apply` path. Non-journal steps keep today's single-step apply. Reuse the #870 flush helper and its test counter so a three-row batch counts as one flush. `append` remains a single-record path (one flush) for callers outside the engine flush batch.

Decisive trade-off: batch only consecutive journals already at the front of the outbox (issue contract) rather than deferring sync inside per-row `apply_journal`, so follow-ups still see a fully durable batch and stay ordered.

## Interfaces / Schema

```rust
pub(crate) fn append_batch(
  dir: &Path,
  lock: &Path,
  records: &[Record],
  now: SystemTime,
) -> Result<Vec<Record>, String>;
```

Beside `append` / `append_locked`. Empty input returns `Ok(vec![])` without locking. Shared `now` keeps the batch on one day file so one sync covers every line.

## Failure modes and edge cases

- Empty batch: no lock, no flush, no follow-ups.
- Write or flush failure: restore the drained journal rows to the front of the outbox and propagate; no follow-ups run for undurable rows.
- Follow-up apply failure after the batch is durable: run `commit_journal` for remaining rows and push unapplied follow-up steps back onto the outbox, then propagate.
- Torn day-file tail: keep existing repair-then-rebuild behavior for the batch write path.
- Concurrent writers: same journal lock as today; other processes do not join the batch.
- Follow-up that enqueues more journals: those land after the current batch's follow-ups finish applying, not inside the already-drained batch.

## Acceptance criteria

- **AC-1:** Given three consecutive journal steps at the front of the outbox, flush assigns consecutive sequences and performs exactly one journal file flush for that batch.
- **AC-2:** Given a batch where row one's commit produces a follow-up step, that follow-up runs after the batch flush and before row two's follow-ups.
- **AC-3:** Existing recover journal scenarios keep passing unchanged.

## Acceptance evidence

- **AC-1:** Unit test installs the flush counter, queues three journal steps (or calls `append_batch` with three records under the engine flush path), asserts sequences `n..n+2` and flush count `1`. Check: `cargo test -p toolu-epic-orchestrator --lib -- journal:: server::`.
- **AC-2:** Same target; a real temporary `Engine` queues two `launch-done` journal steps (empty notes) for issues that already have pending launches so each `commit_journal` yields an `Issue` follow-up. A test-only phase log (same pattern as the #870 flush counter) records `flushed` once after the batch sync and `follow` once per row before those follow-ups apply, asserting `["flushed", "follow", "follow"]`.
- **AC-3:** `cargo test -p toolu-epic-orchestrator --lib -- recover::`.

## Documentation impact

Update `docs/epic-orchestrator/README.md` journal durability wording so a consecutive engine flush batch syncs the day file once after its lines, while a single-record append still syncs once. Touch module comments on `append_batch` / `Engine::flush` as needed. No CLI contract change.

## Open Questions

None. Issue #874 and epic #869 settle the batch boundary; Jev confirmed engine drain + `append_batch` with one end-of-batch sync.
