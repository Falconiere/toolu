# Code-review caller search cache — Plan

**Date:** 2026-10-31   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-31-code-review-caller-search-cache-design.md   **Topic:** Cache raw `git grep HEAD` hits by name per review; compile the definition regex once

## Evidence and approach

Inspected `crates/code-review/src/review/context.rs` (`callers`, `enclosing`, `references`) and its single caller `source.rs::packages`. Existing tests: `review/tests/context_test.rs` (`enclosing`) and `review/tests/source_test.rs` (callers, 20/21 hits, colon paths). Add a private `Callers` struct (compiled regex, raw-hit cache by name, `searches` counter) created in `packages` and passed per file; per-file exclusions and budget run after the cache lookup. Measure `git grep` count and release timing before and after.

## Workstream summary

Baseline measure → cache + tests → after measure → crate tests → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "baseline-measure",
    "title": "Record baseline git grep count and release timing for many files sharing one enclosing name",
    "ac_refs": ["AC-6"],
    "paths": ["crates/code-review/src/review/context.rs", "crates/code-review/src/review/source.rs", "docs/toolu/evidence/2026-10-31-code-review-caller-search-cache/baseline.txt"],
    "input": "temp git repo, 40 files each with fn run changed, one caller file; release build, 5 samples",
    "check": "test -s docs/toolu/evidence/2026-10-31-code-review-caller-search-cache/baseline.txt"
  },
  {
    "id": "cache",
    "title": "Callers struct: regex compiled once, raw hits cached by name, wired through packages",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["baseline-measure"],
    "paths": ["crates/code-review/src/review/context.rs", "crates/code-review/src/review/source.rs", "crates/code-review/src/review/tests/context_test.rs", "crates/code-review/src/review/tests/source_test.rs"],
    "input": "real temp git repos: shared name across files, 20 vs 21 hits, no match, over-64KiB output, colon path, binary file, uncommitted edit, new HEAD",
    "check": "cargo test -p toolu-code-review --lib review::"
  },
  {
    "id": "after-measure",
    "title": "Repeat the release workload after the change and record before/after",
    "ac_refs": ["AC-6"],
    "depends_on": ["cache"],
    "paths": ["crates/code-review/src/review/context.rs"],
    "input": "same workload as baseline-measure",
    "check": "test -s docs/toolu/evidence/2026-10-31-code-review-caller-search-cache/after.txt"
  },
  {
    "id": "crate-tests",
    "title": "Whole toolu-code-review test suite",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["cache"],
    "paths": ["crates/code-review/**"],
    "input": "crate unit and integration tests with real git",
    "check": "cargo test -p toolu-code-review"
  },
  {
    "id": "gate",
    "title": "Full quality gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["crate-tests", "after-measure"],
    "paths": ["crates/code-review/**", "docs/toolu/**"],
    "input": "repository gate on this branch",
    "check": "cargo xtask gate"
  }
]
```

## Critical files

- `crates/code-review/src/review/context.rs`
- `crates/code-review/src/review/source.rs`
- `crates/code-review/src/review/tests/context_test.rs`
- `crates/code-review/src/review/tests/source_test.rs`

## Verification

Same enclosing name in several files runs one `git grep` (`Callers.searches == 1`) with distinct per-file context; boundary cases (20/21, none, failed, cap, budget, colon path, binary, uncommitted, new HEAD) match existing notes. Documentation: none, internal change.
