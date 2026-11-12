# Working-tree `tree_sha` — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-state-tree-sha-design.md   **Topic:** Falconiere/toolu#873 — hash the working tree without touching the index

## Evidence and approach
Inspected issue #873 / epic #869 decisions, `crates/core/state/src/diff_sha.rs` (local
`git` helper, `Option<String>`, `None` on failure), `crates/core/state/src/tests/diff_sha_test.rs`
(real git tempfile fixtures), `crates/core/state/src/lib.rs` exports,
`crates/core/runtime/src/env.rs` (`with`), `crates/core/runtime/src/process.rs`
(`env_clear` when `Spec.env` is set), and `tempfile` already in `toolu-state`
dependencies. Approach: private `tree_sha` module with tempfile `GIT_INDEX_FILE`,
`read-tree HEAD` → `add -A` → `write-tree`; unit tests covering AC-1..AC-6.

## Workstream summary
module + export → real-git unit tests → scoped check → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Add tree_sha module: tempfile GIT_INDEX_FILE, read-tree HEAD, add -A, write-tree; export from lib.rs",
    "check": "test -f crates/core/state/src/tree_sha.rs && rg -n 'pub mod tree_sha' crates/core/state/src/lib.rs && rg -n 'pub fn tree_sha' crates/core/state/src/tree_sha.rs",
    "ac_refs": ["AC-6"],
    "paths": ["crates/core/state/src/tree_sha.rs", "crates/core/state/src/lib.rs", "crates/core/state/src/diff_sha.rs"],
    "input": "approved spec sequence; mirror diff_sha's Spec/env runner; boundary: missing tempfile or failed git step returns None",
    "model": "sonnet"
  },
  {
    "id": "s2",
    "title": "Real-git unit tests for AC-1..AC-6 (stable, tracked edit, untracked, ignored, index/porcelain isolation, None paths)",
    "check": "cargo test -p toolu-state --lib -- tree_sha",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "paths": ["crates/core/state/src/tree_sha.rs", "crates/core/state/src/tests/tree_sha_test.rs", "crates/core/state/src/tests/diff_sha_test.rs"],
    "input": "tempfile repos with real git as named in the spec evidence table; no mocks",
    "model": "sonnet"
  },
  {
    "id": "s3",
    "title": "Scoped quality check for changed state crate and dependents",
    "check": "cargo xtask check-changed --base origin/main",
    "depends_on": ["s2"],
    "ac_refs": ["AC-1", "AC-5"],
    "paths": ["crates/core/state", "crates"],
    "input": "branch diff vs origin/main after s1+s2 commits",
    "model": "haiku"
  },
  {
    "id": "s4",
    "title": "Full quality gate before push",
    "check": "toolu epic job -- cargo xtask gate",
    "depends_on": ["s3"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "paths": ["crates", "docs", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "committed branch head in this worktree",
    "model": "haiku"
  }
]
```

## Critical files
- `crates/core/state/src/tree_sha.rs` (create)
- `crates/core/state/src/tests/tree_sha_test.rs` (create)
- `crates/core/state/src/lib.rs` (export + module blurb)
- `docs/toolu/specs/2026-11-12-state-tree-sha-design.md` (already committed)

## Verification
- End to end: `cargo test -p toolu-state --lib -- tree_sha` proves AC-1..AC-6 on real git.
- Failure: AC-6 paths return `None`; AC-5 proves the real index is untouched.
- Docs: none user-facing; crate module docs only (covered in s1).
- Delivery: scoped commits after green s2; `toolu ledger run <plan> --verify`;
  code-review; `toolu ledger verdict status` ready; push; PR
  `Closes Falconiere/toolu#873` / `Part of Falconiere/toolu#869`; report
  `pr-open` then `babysit` and stop.
