# Epic journal flush — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-epic-journal-flush-design.md   **Topic:** Flush only the JSONL record on append.

## Evidence and approach

Issue #870 and epic #869 designate the JSONL line as durable and both sequence files as rebuildable caches. The pinned base `c533961b2` has no relevant drift on `origin/main`. `journal/index/write.rs` flushes the record; `journal/index.rs` flushes the offsets row, index temp file, and directory; `journal/index/scan.rs` also flushes offsets on rebuild. Existing `journal/index/tests` and `journal/tests/index_test.rs` use real temporary files and writer processes. Keep those conventions and the existing cache consistency/rebuild logic.

## Workstream summary

Add recovery and flush-count tests, make the minimal journal flush change, update module comments, then run scoped and full checks.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Count only the JSONL flush and recover after a truncated day file",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- journal::",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index/write.rs", "crates/epic-orchestrator/src/journal/index/scan.rs", "crates/epic-orchestrator/src/journal/tests", "crates/epic-orchestrator/src/journal/index/tests"],
    "input": "real temporary JSONL and cache files: first and later append count one flush each; two journals truncate a two-record day file to the first record length, then page or append first and verify sequences; existing concurrent writers and torn tails"
  },
  {
    "id": "s2",
    "title": "Check affected crates and journal documentation",
    "check": "cargo xtask check-changed --base origin/main",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/journal/index.rs", "crates/epic-orchestrator/src/journal/index", "crates/epic-orchestrator/src/journal/tests", "docs/toolu/specs/2026-11-12-epic-journal-flush-design.md", "docs/toolu/plans/2026-11-12-epic-journal-flush.md"],
    "input": "the branch diff against origin/main after the journal code and comments change"
  },
  {
    "id": "s3",
    "title": "Run the complete quality gate",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["crates", "docs", "plugins", "fixtures", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "the committed branch against current origin/main"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal/index/write.rs` and its sibling tests
- `crates/epic-orchestrator/src/journal/index.rs`, `journal/index/scan.rs`, and module comments in `journal.rs`
- This plan and its approved design spec

## Verification

The `journal::` target proves the flush count and both truncation recovery paths with real files, alongside existing journal behavior. `check-changed` covers affected crates and docs. The complete `cargo xtask gate` must pass before push. No CLI reference regeneration is needed because there is no command change. Delivery requires a scoped commit, `toolu ledger run <plan> --verify`, a complete version 2 code-review state, `toolu ledger verdict status` reporting ready, and a PR targeting `main` with the issue and epic links. The epic engine owns babysitting and merge after `report babysit`.
