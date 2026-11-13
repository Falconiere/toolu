# Prompt idle epic workers for finished jobs — Plan

**Date:** 2026-11-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-13-epic-idle-job-prompt-design.md   **Topic:** Deliver detached job results when their worker becomes idle.

## Evidence and approach

Issue #879 and epic #869 settle the event, one-time claim, and prompt wording. Merged PR #890 supplies `job_results::claim_finished` and `job_results::line`. `dispatch::reply_event` and `source_apply` have the engine's resource root, while `schedule::note_event` has only `World`. Reuse the result store and `babysit::prompt`; let `note_event` identify `job-done`, then share a helper that checks idle status, claims results, and queues their prompts. Cover both pushed status events and snapshots with real result files.

## Workstream summary

Add failing engine tests, implement the event and idle delivery paths, document the behavior, then run changed and full checks.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Deliver finished results on job-done and idle transitions",
    "check": "cargo test -p toolu-epic-orchestrator --lib job_done && cargo test -p toolu-epic-orchestrator --lib finished_job && cargo xtask check-changed --base origin/main",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "schema-1 result files created and finished under a temporary resource root, a malformed result file, duplicate socket job-done requests, and pushed Herdr events and snapshots carrying working and idle agent statuses"
  },
  {
    "id": "s2",
    "title": "Document idle result delivery",
    "check": "cargo xtask check-markdown-cli",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "the existing epic job documentation and native toolu command tree"
  },
  {
    "id": "s3",
    "title": "Run the complete quality gate",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "the complete branch and real workspace fixtures"
  }
]
```

## Critical files

`crates/epic-orchestrator/src/schedule.rs`, `dispatch.rs`, `source_apply.rs`, their sibling tests (including `tests/dispatch_job_test.rs`), and `docs/epic-orchestrator/README.md`.

## Verification

The focused tests use the real result store and engine journal to prove one-time delivery, deferred delivery while working, snapshot delivery, and prompt text. Run `check-changed` after code work, then the full gate before pushing. Fetch and rebase if `origin/main` moves; rerun affected checks. Final delivery needs ledger verification, complete code review, ready verdict, push, and the PR. After `report babysit`, the epic engine owns further review and merge.

## Deviations

The socket-event test moved to its own sibling module to keep `dispatch_test.rs` under the Rust file limit. The first full crate run exposed an ordering assumption in the new two-result test; it was corrected. Two unchanged rebase fault tests also failed in that concurrent run, then all six rebase fault tests passed on both this branch and an isolated `origin/main` checkout under one managed job. The first `check-changed` found duplicated test setup and missing pinned npm tools; the second found three Clippy test syntax issues. After consolidating the setup, running `npm ci --ignore-scripts`, and fixing those assertions and match arms, `check-changed` passed.
