# Epic journal flush — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Codex   **Topic:** Flush the journal record without flushing rebuildable caches.

## Problem

Each journal append currently flushes the JSONL record and up to three cache files or directories. The durable record is the JSONL line; cache flushes add disk cost without preserving additional source data.

## Non-Goals

1. Change how the JSONL line is flushed or batch records.
2. Change cache formats, locking, sequence allocation, or public journal interfaces.

## Architecture

Keep the JSONL `sync_all` in `journal/index/write.rs`, routed through one small helper with a test-only thread-local counter; it is the only production journal flush. Remove cache flushes from `journal/index.rs` and `journal/index/scan.rs`, while keeping writes, the summary rename, and existing error propagation. The initial `fresh -> rebuild` path also writes `sequence.offsets`, so its flush must be removed to satisfy the first-append count. `consistent` and `rebuild` remain the recovery path when caches disagree with JSONL.

## Interfaces / Schema

No external interface or schema changes. The helper accepts `&File` and returns `Result<(), String>`; its counter is installed only by unit tests. `sequence.index`, `sequence.offsets`, and daily JSONL formats stay the same.

## Failure modes and edge cases

JSONL write or flush failure still propagates before cache publication. Cache write or rename failure still propagates. A missing, stale, or corrupt cache is rebuilt from complete JSONL records. If a day file is truncated after an append, the next append and page must discard the stale cache and continue from the log's last sequence. Concurrent writers retain the existing journal lock.

## Acceptance criteria

- **AC-1:** Given an empty journal and one real append, exactly one journal file flush occurs; the same is true for a later append.
- **AC-2:** Given a day file truncated to its length before a completed append, the next append and page rebuild the cache and resume from the last sequence present in JSONL.
- **AC-3:** Given existing concurrent-writer, torn-tail, cache-removal, and paged-read scenarios, append and page return the same ordered records and sequences.

## Acceptance evidence

- **AC-1:** A unit test installs the thread-local counter, appends to a temporary journal twice, and asserts one flush for each append. Run `cargo test -p toolu-epic-orchestrator --lib -- journal::`.
- **AC-2:** Two independent temporary journals each receive two records and are truncated to the first record's prior length. One is paged immediately and must return only sequence 1; the other is appended and must reuse sequence 2, then page must return sequences 1 and 2. Run the same target.
- **AC-3:** The existing journal unit tests exercise real temporary JSONL files, cache files, and writer processes; the same target must pass all of them unchanged.

## Documentation impact

Update the journal module comments to state that only the JSONL record is flushed and the cache is rebuildable. No CLI or user-facing command changes.

## Open Questions

None. Issue #870 and epic #869 settle the durability boundary; the additional rebuild flush was found in current source and follows the same cache rule.
