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
    "id": "s2",
    "title": "Use candidate scans for notify, report, and answer deduplication",
    "check": "cargo test -p toolu-epic-orchestrator --lib",
    "depends_on": ["s1"],
    "ac_refs": ["AC-2", "AC-3", "AC-4"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs", "crates/epic-orchestrator/src/tests/dispatch_notify_replay_test.rs", "crates/epic-orchestrator/src/tests/attention_submission_test.rs", "crates/epic-orchestrator/src/tests/attention_ops_test.rs", "crates/epic-orchestrator/src/metrics", "crates/epic-orchestrator/src/cleanup.rs"],
    "input": "real temporary engine journals spanning older, yesterday, and today dates; token in note versus token field; escaped submission ID and existing metrics, cleanup, dispatch cases"
  },
  {
    "id": "s3",
    "title": "Document journal read behavior and run changed checks",
    "check": "env -u PUSH_REVIEW_BASE cargo xtask check-changed --base origin/main && env -u PUSH_REVIEW_BASE cargo xtask check-markdown-cli",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/journal/tests", "crates/epic-orchestrator/src/tests", "docs/epic-orchestrator/README.md", "docs/toolu/specs/2026-11-12-epic-journal-read-design.md", "docs/toolu/plans/2026-11-12-epic-journal-read.md"],
    "input": "the branch diff against origin/main, including the owning epic guide and reviewed spec"
  },
  {
    "id": "s4",
    "title": "Run the complete quality gate",
    "check": "env -u PUSH_REVIEW_BASE cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-4"],
    "input": "the committed branch against current origin/main"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal.rs` and `crates/epic-orchestrator/src/tests/journal_test.rs`
- `crates/epic-orchestrator/src/dispatch.rs`, `attention_ops.rs`, and their sibling tests
- `docs/epic-orchestrator/README.md`, this plan, and its approved spec

## Verification

The focused journal test proves the wire-format edge cases on real JSONL files; the crate lib suite covers affected callers and the existing metrics, cleanup, and dispatch cases. `check-changed` and the full gate check affected and workspace behavior. Before push, fetch and rebase if main moved and rerun affected checks. Delivery requires scoped commits, final `toolu ledger run <plan> --verify`, a complete version 2 code-review state, `toolu ledger verdict status` of `ready`, and a PR targeting `main` with issue and epic links. The epic engine takes over after `report babysit`.
