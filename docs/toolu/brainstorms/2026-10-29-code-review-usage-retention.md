# Code-review usage retention — Brainstorm

**Date:** 2026-10-29   **Mode:** Delivery (full)   **Issue:** Falconiere/toolu#750

## Outcome

Keep the project review usage log bounded while preserving complete JSONL rows in every retained file. A write, rotation, or cleanup failure blocks the review's passing push state.

## Evidence and choice

`pipeline/state.rs::log_usage` currently appends one row to an unbounded file. `persist` already adds a blocker when the write fails. `shape/dirty.rs` excludes that exact file. `toolu_state::lock::with_lock_strict` provides a shared strict lock protocol; use it with a long stale interval so a live cleanup cannot lose ownership. Jev favored size-based rotation over age-only retention or rewriting a single file (0.92 relative probability).

Rotate before a row would exceed 1 MiB, retain four archived generations plus the active file, and reject a row larger than 1 MiB. Keep archives in a dedicated `code-review-usage/` directory with monotonically numbered names. Delete the oldest complete archives under the same lock after appending. An already oversized legacy active file is outside the new retention limit and is reclaimed on its first rotation.

## Boundaries and risk

No new CLI, configuration, compression, or change to the usage JSON shape. The active file and archives are read after reviews quiesce, oldest archive first. A failed append must truncate a partial row back to the old length; failed rollback still blocks the review and reports both errors. Existing oversized logs may be reclaimed at first rotation under the documented policy. Concurrent writers must serialize before inspecting sizes or deleting archives. The dirty-tree filter excludes the exact active file and lock, plus archived JSONL files; unrelated files inside that directory remain visible to Git.

## Handoff

Specify these observable outcomes and real-file tests in the design, then plan and implement.
