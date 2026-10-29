# Derive subprocess stdout views from one byte snapshot — Design

**Date:** 2026-10-29   **Status:** Approved   **Author:** epic-worker-756   **Topic:** One stdout byte snapshot for `Output` text and bytes

## Problem

`process::run` builds `Output` by calling `Drain::text()` then `Drain::bytes()`, each locking the drain buffer separately. Text and bytes can theoretically disagree if the buffer changed between calls, and the dual read is an unnecessary second walk for large bounded stdout. Issue #756 asks for one byte snapshot with text derived from those same bytes, without a broad process refactor.

## Non-Goals

1. Changing public `Output` field names or types (`stdout: String`, `stdout_bytes: Vec<u8>` stay).
2. Adding `stderr_bytes` or rewriting stderr handling (stderr remains text-only).
3. Pipe cancellation, drain threading, budget, timeout, or wait-mode changes (#746 and siblings).
4. Sharing one allocation between `String` and `Vec` via `Arc` or changing callers to a single view.
5. Inventing a speedup target; record measured before/after allocation counts only.

## Architecture

At `Output` construction in `crates/core/runtime/src/process.rs`, take one `stdout.bytes()` clone, then set `stdout` with `String::from_utf8_lossy(&stdout_bytes).into_owned()`. Leave `Drain::text` / `Drain::bytes` for stderr and existing drain unit tests. Decisive trade-off: smallest construction-site change vs a new `Drain::snapshot` helper — prefer the site change (Jev `inline_at_site`) so drain API surface stays put.

Reuse: existing `Drain`, `Budget`, lossy UTF-8 path, and `process_test` / `drain_test` coverage.

## Interfaces / Schema

Unchanged public surface:

```rust
pub struct Output {
  pub pid: u32,
  pub stdout: String,
  pub stdout_bytes: Vec<u8>,
  pub stderr: String,
  pub exit_code: i32,
  pub duration: Duration,
  pub timed_out: bool,
  pub truncated: bool,
}
```

Construction (conceptual):

```rust
let stdout_bytes = stdout.bytes();
let stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();
```

## Failure modes and edge cases

| Case | Behavior |
| --- | --- |
| Empty stdout | Both fields empty; unchanged. |
| Invalid UTF-8 | `stdout_bytes` exact; `stdout` uses U+FFFD replacements (same as today). |
| At/above `max_output_bytes` | Truncation flag and kept prefix unchanged; text derived from kept bytes only. |
| Timeout with partial stdout | Partial bytes + derived text; `timed_out` unchanged. |
| stderr vs stdout | stderr still via `text()` only; no cross-stream bleed. |
| Drain lock poison | Existing `lock` unwraps to inner; unchanged. |

## Acceptance criteria

- **AC-1:** After a real subprocess writes valid UTF-8 stdout, `output.stdout_bytes` equals those bytes and `output.stdout == String::from_utf8_lossy(&output.stdout_bytes)` (asserted in a process test).
- **AC-2:** After a real subprocess writes invalid UTF-8 stdout, `stdout_bytes` is byte-exact and `stdout` matches today’s lossy decoding (existing `stdout_bytes_keep_what_the_lossy_text_replaces` stays green; same lossy-equality assert as AC-1).
- **AC-3:** Existing real-subprocess tests stay green without behavior edits: `a_command_reports_its_streams_and_exit_code` (stderr separation + empty truncation flags), `output_past_the_budget_is_dropped` (cap), `the_deadline_terminates_the_whole_group` / streams-mode timeout tests (timeout), and empty-stdout via `true` or equivalent remains empty bytes and text.
- **AC-4:** Before editing construction, measure baseline on this branch’s pre-change `process::run` with N≥5 release runs of ~1 MiB under-cap stdout; after the one-snapshot edit, repeat the same probe. PR body records platform, workload, N, and before→after wall and/or peak RSS. Diff at `Output` construction must show `let stdout_bytes = …; let stdout = String::from_utf8_lossy(&stdout_bytes)…` (or equivalent). No invented speedup target.

## Acceptance evidence

| AC | Real input | Expected | Check |
| --- | --- | --- | --- |
| AC-1 | `printf 'hello'` via `process::run` | bytes `b"hello"`, text `"hello"`, and `stdout == String::from_utf8_lossy(&stdout_bytes)` | new/extended `process_test` + `cargo test -p toolu-runtime` |
| AC-2 | `printf 'a\377\376b'` via existing test | bytes keep `\xff\xfe`; text `a\u{fffd}\u{fffd}b`; same lossy-equality | `stdout_bytes_keep_what_the_lossy_text_replaces` |
| AC-3 | Named tests above with real `sh`/`head` children | same assertions as today | `cargo test -p toolu-runtime` |
| AC-4 | Release probe before then after the construction edit | numbers in PR body; one-snapshot site in diff | `/usr/bin/time -l` (Darwin) or `time -v` (Linux); `cargo xtask gate` |

## Documentation impact

None — internal construction change; public `Output` docs already describe lossy text and exact bytes. No CLI or skill surface change.

## Open Questions

None blocking. Decision recorded: stdout-only (Jev noul 0.18 against stderr change); construction-site derive (Jev choice `inline_at_site`).
