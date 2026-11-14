# macOS journal fsync — Plan

**Date:** 2026-11-14   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-14-macos-journal-fsync-design.md   **Topic:** Reduce macOS journal append latency.

## Evidence and approach

Issue #875 names `nix::unistd::fsync` on macOS and existing `File::sync_all` elsewhere. The current `journal/index.rs::flush` is the only production JSONL flush and owns the counter. The crate already depends on `nix` with `fs`. Issue #870's counter and truncation tests use real temporary files. The pinned base differs because later epic changes batch writes, but they still call the same helper once per nonempty batch.

## Workstream summary

Add a small real-file timing test and measure the current append path, change the shared flush helper, document the guarantee, measure the changed path with the same test, and verify the branch. The timing test asserts the final sequence but does not impose a machine-dependent speed threshold.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Use plain fsync for macOS journal appends",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- journal::",
    "ac_refs": ["AC-1"],
    "paths": ["crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index/write.rs", "crates/epic-orchestrator/src/journal/index/tests", "crates/epic-orchestrator/src/journal/tests"],
    "input": "real temporary daily JSONL files for first and later appends, batches, stale caches and truncated logs; expect one flush and intact recovery"
  },
  {
    "id": "s2",
    "title": "Document the macOS durability boundary",
    "check": "cargo xtask check-markdown-cli",
    "depends_on": ["s1"],
    "ac_refs": ["AC-2"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "docs/epic-orchestrator/README.md", "docs/toolu/specs/2026-11-14-macos-journal-fsync-design.md", "docs/toolu/plans/2026-11-14-macos-journal-fsync.md"],
    "input": "module and epic README text describing process crash or kill recovery and possible power-cut loss"
  },
  {
    "id": "s3",
    "title": "Check changed code and append-time evidence",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- journal::index::write::tests::append_timing --nocapture && env -u PUSH_REVIEW_BASE cargo xtask check-changed --base origin/main",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index/write.rs", "crates/epic-orchestrator/src/journal/index/tests", "crates/epic-orchestrator/src/journal.rs", "docs/epic-orchestrator/README.md"],
    "input": "repeated real appends to a temporary daily JSONL file on the same Mac before and after, with printed per-append timing and final sequence assertion"
  },
  {
    "id": "s4",
    "title": "Run the complete quality gate",
    "check": "env -u PUSH_REVIEW_BASE cargo xtask gate",
    "depends_on": ["s3"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "committed branch diff against current origin/main"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal/index.rs`
- `crates/epic-orchestrator/src/journal/index/tests/write_test.rs`
- `crates/epic-orchestrator/src/journal.rs`
- `docs/epic-orchestrator/README.md`
- This plan and its spec

## Verification

Measure real repeated appends before and after using the same real-file timing test and report both measurements in the PR. Journal tests cover one flush, batches, and cache recovery. Run `check-changed`, markdown CLI check, and the full gate. Commit, run final ledger verification and code review, confirm the verdict is ready, push, and open a PR targeting `main`; the epic engine handles babysitting and merge.
