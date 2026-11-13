# Epic journal reads — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Codex   **Topic:** Parse journal rows directly and skip irrelevant token rows before parsing.

## Problem

Journal reads build a `serde_json::Value` tree for every row. Notify token lookup scans all retained rows, while report and answer deduplication parse two days of rows even when almost none carry the requested token.

## Non-Goals

1. Add a token cache or change the journal's JSONL format, retention, or write path.
2. Change the all-retained notify window or the yesterday-and-today report and answer windows.

## Architecture

Deserialize directly into `Record`: `seq` remains a required unsigned integer, and a shared deserializer maps missing or non-string text fields to empty strings. Keep the existing daily-file reader and date ordering. Give it an optional token-text filter so `each_with(dir, from_day, needle, each)` skips complete lines without the needle before `parse`; `each` retains its current behavior. JSON-encode the needle before the raw-line check because an answer submission ID may contain escaped characters. Callers still compare the decoded `row.token` exactly, so a token in `note` cannot match. Notify uses `each_with` for all retained days; report and answer use `tail_with(dir, now, needle, each)` to read exactly yesterday and today, excluding future-dated files.

## Interfaces / Schema

Add `journal::each_with(&Path, &str, &str, FnMut(Record) -> Result<(), String>) -> Result<(), String>` and `journal::tail_with(&Path, SystemTime, &str, FnMut(Record) -> Result<(), String>) -> Result<(), String>`. The `from_day` argument includes that UTC date and later retained day files. `tail_with` reads the same two named files as `tail`. The needle is a token's decoded text; an empty needle scans all rows. No external command, config, or on-disk schema changes.

## Failure modes and edge cases

Malformed JSON, incomplete final lines, and rows without a valid `u64` `seq` are skipped. Missing or non-string text fields become empty, including numeric or nested `note`. A token appearing only in another row's `note` is parsed as a candidate but does not count. Escaped token characters remain searchable. A missing journal directory is an empty scan; directory or file read errors and callback errors propagate. The existing journal lock and append path are untouched.

## Acceptance criteria

- **AC-1:** Given complete JSONL rows with a numeric `note`, missing text fields, or missing `seq`, a journal read returns the numeric-note row with empty text values and skips the missing-sequence row.
- **AC-2:** Given a token present in one row's `note` and another row's `token`, notify, report, and answer deduplication recognize only the matching token field, including an answer submission ID requiring JSON escaping.
- **AC-3:** Given older and future-dated records outside the two-day window, notify lookup still scans all retained days while report and answer lookup consider only yesterday and today; malformed and partial rows do not change those results.
- **AC-4:** Existing metrics, cleanup, dispatch, and journal behavior remains green with real temporary JSONL files and the native Rust test binary.

## Acceptance evidence

- **AC-1:** Plant complete JSONL rows in a temporary journal with `"note":123`, absent `name`, and no `seq`, then assert `tail`/`each` output. Run `cargo test -p toolu-epic-orchestrator --lib -- journal::`.
- **AC-2:** Plant and append real records with a searched token in a different row's note, a matching spool or admission row, and an ID containing quotes or a newline. Assert `spool_seen` and answer retry behavior through the existing engine test harness. Run `cargo test -p toolu-epic-orchestrator --lib -- dispatch` and `cargo test -p toolu-epic-orchestrator --lib -- attention`.
- **AC-3:** Plant day files on the prior, current, older, and future UTC dates and a torn line, then assert the distinct scan windows in journal and dispatch/answer tests. Run the same focused targets.
- **AC-4:** Run `cargo test -p toolu-epic-orchestrator --lib -- metrics`, `cargo test -p toolu-epic-orchestrator --lib -- cleanup`, and `cargo test -p toolu-epic-orchestrator --lib -- dispatch`, followed by `cargo xtask gate` before push.

## Documentation impact

Update `docs/epic-orchestrator/README.md` to describe direct row parsing and token prefiltering, without promising an on-disk index or a changed retention window.

## Open Questions

None. Issue #871 and epic #869 settle the observable behavior and scope; current source confirms the parser and call sites. The JSON-encoded needle preserves arbitrary submission IDs without changing the exact token comparison. `tail` reads two named files, so `tail_with` preserves that boundary when future-dated files exist.
