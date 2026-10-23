# finish --abandon settles an unstarted launch — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-finish-abandon-unstarted-design.md   **Topic:** abandon a launch that never created a worktree (#584)

## Evidence and approach
`finish/record.rs` rejects records without `checkout`/`worktree`; `finish.rs` always tears down. Add `never_started`, relax the validator for that shape, and skip the teardown when abandoning it, reusing `settle` and `release_recorded_ownership`.

## Workstream summary
tests (red) → record validation → finish skip + dry run → module doc.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "Red-first tests for unstarted abandon, dry run, refusals, repeat", "check": "cargo test -p toolu-epic-orchestrator finish::tests::unstarted", "ac_refs": ["AC-1","AC-2","AC-3","AC-4","AC-5"], "paths": ["crates/epic-orchestrator/src/tests/finish_test.rs","crates/epic-orchestrator/src/tests/finish_rig_test.rs"], "input": "finish rig record with no worktree and a real agent lease", "model": "inherit"},
  {"id": "s2", "title": "never_started and relaxed read_issue_record", "check": "cargo test -p toolu-epic-orchestrator finish", "ac_refs": ["AC-2","AC-4"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/finish/record.rs","crates/epic-orchestrator/src/finish/tests/record_test.rs"], "input": "records with checkout only and worktree only", "model": "inherit"},
  {"id": "s3", "title": "Skip teardown and plan lease release for unstarted abandon; update module doc", "check": "cargo test -p toolu-epic-orchestrator finish", "ac_refs": ["AC-1","AC-3","AC-5"], "depends_on": ["s2"], "paths": ["crates/epic-orchestrator/src/finish.rs","crates/epic-orchestrator/src/tests/finish_test.rs"], "input": "uncertain record plus agent lease", "model": "inherit"}
]
```

## Critical files
`crates/epic-orchestrator/src/finish.rs`, `finish/record.rs`, `tests/finish_test.rs`, `finish/tests/record_test.rs`, `tests/finish_rig_test.rs`.

## Verification
`cargo test -p toolu-epic-orchestrator finish`, then `cargo xtask gate`. Docs: None (spec).
