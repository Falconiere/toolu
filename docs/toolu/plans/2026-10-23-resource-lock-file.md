# Resource lock file diagnostic — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-resource-lock-file-design.md   **Topic:** Diagnose a regular file at the lock path.

## Evidence and approach

Issue #590 shows `state.lock/owner.json: Not a directory`. `crates/core/engine/src/resources/lock.rs` reads that child path while reclaiming an existing lock. Its sibling tests exercise real directories and preserve unknown ownership. Add a guarded type check that reports the offending path and safe manual action while leaving it intact.

## Workstream summary

Prove the failure with a real file, implement the diagnostic, then document recovery and run the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "lock-file",
    "title": "Diagnose and preserve a regular lock file",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["crates/core/engine/src/resources/lock.rs", "crates/core/engine/src/resources/tests/lock_test.rs"],
    "input": "Temporary resource home containing a nonempty state.lock regular file, plus existing directory lock cases",
    "check": "cargo test -p toolu-engine resources::lock::tests"
  },
  {
    "id": "recovery-doc",
    "title": "Document the regular-file error and recovery",
    "ac_refs": ["AC-1"],
    "depends_on": ["lock-file"],
    "paths": ["docs/epic-orchestrator/README.md"],
    "input": "The exact error wording emitted for a regular state.lock file",
    "check": "cargo xtask check-markdown-cli"
  }
]
```

## Critical files

- `crates/core/engine/src/resources/lock.rs`
- `crates/core/engine/src/resources/tests/lock_test.rs`
- `docs/epic-orchestrator/README.md`

## Verification

The focused real-filesystem test must fail before implementation and pass after it. Run `cargo xtask gate` through `toolu epic job --` before pushing. Before delivery, commit scoped changes, run `toolu ledger run docs/toolu/plans/2026-10-23-resource-lock-file.md --verify`, complete the version 2 review and ready verdict, then push and open a PR against `main`. Rebase on moved `origin/main` and recheck affected results before implementation and push.
