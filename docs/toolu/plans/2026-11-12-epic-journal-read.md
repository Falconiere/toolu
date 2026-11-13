# Epic journal reads — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-epic-journal-read-design.md   **Topic:** Parse rows directly and avoid parsing rows irrelevant to token lookups.

## Evidence and approach

Issue #871 pins `c533961b2`; the intervening #889 merge changes only journal cache flushes and its documentation, leaving `parse`, `tail`, `each`, `dispatch::spool_seen`, and `attention_ops::prior` intact. `journal.rs` currently parses `Value` and `read_file` holds one day at a time. `src/tests/journal_test.rs`, `src/tests/dispatch_test.rs`, `src/tests/dispatch_notify_replay_test.rs`, and `src/tests/attention_submission_test.rs` use real temporary journal files and engine calls. Reuse those paths. Derive direct `Record` deserialization with one lossy-string visitor, prefilter complete lines using the JSON-encoded needle, and keep exact `row.token` comparisons and existing windows.

## Workstream summary

Establish parser and filter behavior with tests, switch the three token lookups, synchronize the epic guide, then run scoped and complete checks.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Parse journal records directly and filter candidate lines",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- journal::",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/tests/journal_test.rs", "crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index/read.rs"],
    "input": "temporary daily JSONL files with numeric and missing text fields, missing sequence, escaped token, other-row note, and incomplete tail; assert parsed fields, date order, and candidate filtering"
  },
  {
    "id": "s1a",
    "title": "Repair the existing duplicate-code gate finding",
    "check": "cargo test -p toolu-ast-grep --lib -- map::large && cargo xtask gate --only jscpd",
    "depends_on": ["s1"],
    "paths": ["crates/ast-grep/src/map/large.rs", "crates/ast-grep/src/map/tests/large_test.rs"],
    "input": "the existing large-directory text and JSON summaries with 160 real FileOutline entries; jscpd scans repository Rust source"
  },
  {
    "id": "s2",
    "title": "Use candidate scans for notify, report, and answer deduplication",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- --test-threads=4",
    "depends_on": ["s1a"],
    "ac_refs": ["AC-2", "AC-3", "AC-4"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs", "crates/epic-orchestrator/src/tests/dispatch_notify_replay_test.rs", "crates/epic-orchestrator/src/tests/attention_submission_test.rs", "crates/epic-orchestrator/src/tests/attention_window_test.rs", "crates/epic-orchestrator/src/tests/attention_ops_test.rs", "crates/epic-orchestrator/src/metrics", "crates/epic-orchestrator/src/cleanup.rs"],
    "input": "real temporary engine journals spanning older, yesterday, today, and future dates; token in note versus token field; escaped submission ID and existing metrics, cleanup, dispatch cases"
  },
  {
    "id": "s3",
    "title": "Document journal read behavior and run changed checks",
    "check": "env -u PUSH_REVIEW_BASE NEXTEST_TEST_THREADS=4 cargo xtask check-changed --base origin/main && env -u PUSH_REVIEW_BASE cargo xtask check-markdown-cli",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/journal/tests", "crates/epic-orchestrator/src/tests", "docs/epic-orchestrator/README.md", "docs/toolu/specs/2026-11-12-epic-journal-read-design.md", "docs/toolu/plans/2026-11-12-epic-journal-read.md"],
    "input": "the branch diff against origin/main, including the owning epic guide and reviewed spec"
  },
  {
    "id": "s4",
    "title": "Run the complete quality gate",
    "check": "env -u PUSH_REVIEW_BASE NEXTEST_TEST_THREADS=4 cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-4"],
    "input": "the committed branch against current origin/main"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal.rs` and `crates/epic-orchestrator/src/tests/journal_test.rs`
- `crates/ast-grep/src/map/large.rs` and its sibling test for the gate repair
- `crates/epic-orchestrator/src/dispatch.rs`, `attention_ops.rs`, and their sibling tests
- `docs/epic-orchestrator/README.md`, this plan, and its approved spec

## Verification

The focused journal test proves the wire-format edge cases on real JSONL files; the crate lib suite covers affected callers and the existing metrics, cleanup, and dispatch cases. `check-changed` and the full gate check affected and workspace behavior. Before push, fetch and rebase if main moved and rerun affected checks. Delivery requires scoped commits, final `toolu ledger run <plan> --verify`, a complete version 2 code-review state, `toolu ledger verdict status` of `ready`, and a PR targeting `main` with issue and epic links. The epic engine takes over after `report babysit`.

## Deviations

After s1, `cargo xtask check-changed --base origin/main` failed on an 11-line jscpd duplicate within `crates/ast-grep/src/map/large.rs` that is already present on main. The same run reported missing web tools in this worktree. Installed the pinned tools with `npm ci --ignore-scripts` and added s1a to remove the duplicate through one shared directory-summary calculation, test both outputs, and rerun jscpd before continuing. This is a gate repair; journal behavior and acceptance criteria are unchanged.

Before s2, source inspection showed that `each_with(from_day)` includes future-dated files while the existing `tail` reads exactly yesterday and today. The reviewed spec now names `tail_with` for report and answer, and s2's input includes a future-day row. The issue's unchanged-window requirement is preserved.

The s2 `check-changed` run found `attention_submission_test.rs` at 301 code lines, one above the 300-line guardrail. Moved its future-day test unchanged in behavior to sibling `attention_window_test.rs` and registered that test module. The test stays in s2's declared paths; no assertion or acceptance criterion was removed.

Two broad runs on the shared machine failed different unchanged tests under load: Nextest's CLI doctor stub did not report its version after ten seconds, and an asynchronous epic notification missed its five-second wait. Each test passed when rerun alone; the full epic library suite also passed before the test-file move. The remaining suite checks cap test threads at four to retain all assertions while reducing contention. The final gate remains required to pass.
