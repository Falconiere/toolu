# macOS journal fsync — Design

**Date:** 2026-11-14   **Status:** Approved   **Author:** Codex   **Topic:** Reduce journal append latency on macOS

## Problem

The journal's `sync_all` flush is expensive on macOS for every append. Issue #875 requires a plain `fsync` while retaining one flush per append.

## Non-Goals

1. Change Linux or other platform flush behavior.
2. Flush state files or rebuildable journal caches.

## Architecture

Use the existing `journal::index::flush` helper, its test counter, and the crate's existing `nix` dependency. On macOS call `nix::unistd::fsync` on the file descriptor; on other platforms call `File::sync_all`. The journal writer and cache update order stay intact. The macOS trade-off is lower latency with no power-cut durability promise for the latest rows.

## Interfaces / Schema

`fn flush(file: &std::fs::File) -> Result<(), String>` keeps its signature and error mapping. No CLI, data format, or dependency changes.

## Failure modes and edge cases

Empty batches still do not flush. One or more rows in a nonempty batch flush the daily file once before cache updates. A flush error propagates as a string and prevents cache publication. Incomplete tails and stale caches retain their current recovery behavior. On macOS a process crash or kill leaves flushed rows recoverable, while a power cut may lose the latest rows.

## Acceptance criteria

- **AC-1:** On macOS a real append uses plain `fsync` once; on other platforms it uses `sync_all` once. A nonempty batch also flushes once.
- **AC-2:** The module doc and epic README state the macOS process-crash and power-cut guarantee accurately.
- **AC-3:** Repeated real journal appends on this Mac have measured per-append time before and after the change, reported in the PR.

## Acceptance evidence

- **AC-1:** The existing `each_append_flushes_only_its_jsonl_line` and batch counter tests use real temporary files and expect one flush; `cargo test -p toolu-epic-orchestrator --lib -- journal::` checks them and recovery. Source inspection confirms the platform branch and errors.
- **AC-2:** Review `crates/epic-orchestrator/src/journal.rs` and `docs/epic-orchestrator/README.md`; run `cargo xtask check-markdown-cli`.
- **AC-3:** Run `cargo test -p toolu-epic-orchestrator --lib -- journal::index::write::tests::append_timing --nocapture` on the current base and changed code on this Mac. Its real temporary journal file, final sequence assertion, sample count, and printed per-append time provide the PR measurements.

## Documentation impact

Update the journal module doc and `docs/epic-orchestrator/README.md`.

## Open Questions

None. The issue and epic settle the implementation and guarantee.
