# Epic graph clone root — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-epic-graph-clone-root-design.md   **Topic:** resolve clone_root from roots; clone once

## Evidence and approach
`from_tracker.rs` falls back to `input.cwd`; `launch/checkout.rs::clone_checkout` clones unguarded. Drop `cwd`, add a resolver, lock the clone with `toolu_state::lock::with_lock`.

## Workstream summary
resolver → clone lock → full gate.

## Steps (machine-readable)
```json
[
  {"id": "resolve", "title": "clone_root from roots, never cwd; refuse / or unresolved", "check": "cargo test -p toolu-epic-orchestrator from_tracker", "ac_refs": ["AC-1", "AC-2", "AC-3"], "paths": ["crates/epic-orchestrator/src/graph", "crates/epic-orchestrator/src/effects_epic.rs", "crates/epic-orchestrator/src/verbs_actions/graph.rs"], "input": "Jira fixture tracker with temp roots", "model": "inherit"},
  {"id": "clone-once", "title": "lock and skip existing clone", "check": "cargo test -p toolu-epic-orchestrator checkout", "ac_refs": ["AC-4"], "depends_on": ["resolve"], "paths": ["crates/epic-orchestrator/src/launch/checkout.rs", "crates/epic-orchestrator/src/launch/tests/checkout_test.rs"], "input": "two threads, stub gh on PATH", "model": "inherit"}
]
```

## Critical files
graph/from_tracker.rs, effects_epic.rs, verbs_actions/graph.rs, launch/checkout.rs and their tests.

## Verification
`cargo test -p toolu-epic-orchestrator`; `cargo xtask gate`. Docs: none (spec).
