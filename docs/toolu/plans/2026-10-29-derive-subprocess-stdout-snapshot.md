# Derive subprocess stdout views from one byte snapshot — Plan

**Date:** 2026-10-29   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-29-derive-subprocess-stdout-snapshot-design.md   **Topic:** One stdout byte snapshot for `Output` text and bytes

## Evidence and approach

Issue #756 / approved spec: `Output` construction in `crates/core/runtime/src/process.rs` calls `stdout.text()` then `stdout.bytes()` (two locks). Change the site to `let stdout_bytes = stdout.bytes(); let stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();`. Keep public `Output` fields; leave stderr and `Drain` helpers alone. Existing `process_test` covers invalid UTF-8, truncation, timeout, stderr; extend with valid UTF-8 + lossy-equality. Measure release probe before then after. Local-gate protocol (orchestrator): ledger+gate green on pushed head; dual independent `toolu-review:review` + `write-state reviewers=["toolu-review:review"]`; report `ready`; no pr-babysit, no merge.

## Workstream summary

Baseline measure → construction edit + tests → after measure → runtime tests + gate → deliver PR → dual review → report ready.

## Steps (machine-readable)

```json
[
  {
    "id": "baseline-measure",
    "title": "Release probe before edit: N>=5 runs of ~1MiB under-cap stdout",
    "ac_refs": ["AC-4"],
    "paths": ["crates/core/runtime/src/process.rs", "crates/core/runtime/src/tests/process_test.rs"],
    "input": "release process::run ~1MiB stdout x5 via one-off cargo test harness before construction edit; Darwin /usr/bin/time -l",
    "check": "test -s .claude/tmp/756-measure/baseline.txt"
  },
  {
    "id": "derive-stdout",
    "title": "Construct Output stdout from one byte snapshot",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["baseline-measure"],
    "paths": ["crates/core/runtime/src/process.rs", "crates/core/runtime/src/tests/process_test.rs"],
    "input": "printf hello and printf invalid UTF-8 via real sh subprocess",
    "check": "cargo test -p toolu-runtime --lib stdout_bytes_keep_what_the_lossy_text_replaces"
  },
  {
    "id": "after-measure",
    "title": "Repeat release probe after edit; record before/after in notes",
    "ac_refs": ["AC-4"],
    "depends_on": ["derive-stdout"],
    "paths": ["crates/core/runtime/src/process.rs"],
    "input": "same workload as baseline-measure",
    "check": "test -s .claude/tmp/756-measure/after.txt"
  },
  {
    "id": "runtime-coverage",
    "title": "Full toolu-runtime process/drain coverage",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "depends_on": ["derive-stdout"],
    "paths": ["crates/core/runtime/src/process.rs", "crates/core/runtime/src/process/drain.rs", "crates/core/runtime/src/tests/process_test.rs", "crates/core/runtime/src/process/tests/drain_test.rs"],
    "input": "crate unit and integration process tests with real subprocesses",
    "check": "cargo test -p toolu-runtime"
  },
  {
    "id": "gate",
    "title": "Full quality gate under epic job lease",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["runtime-coverage", "after-measure"],
    "paths": ["crates/core/runtime/**", "docs/toolu/specs/2026-10-29-derive-subprocess-stdout-snapshot-design.md", "docs/toolu/plans/2026-10-29-derive-subprocess-stdout-snapshot.md"],
    "input": "repository gate on this branch",
    "check": "toolu epic job -- cargo xtask gate"
  }
]
```

## Critical files

`crates/core/runtime/src/process.rs`, `crates/core/runtime/src/tests/process_test.rs`, `docs/toolu/specs/2026-10-29-derive-subprocess-stdout-snapshot-design.md`, `docs/toolu/plans/2026-10-29-derive-subprocess-stdout-snapshot.md`.

## Verification

Real-subprocess process tests prove lossy-equality and preserve truncation/timeout/stderr. Release probe numbers go in the PR body. `toolu ledger run … --verify`, dual independent `toolu-review:review` with `write-state`, push, open PR `Closes Falconiere/toolu#756` / `Part of Falconiere/toolu#737`, then `report ready` (local-gate; no babysit/merge).
