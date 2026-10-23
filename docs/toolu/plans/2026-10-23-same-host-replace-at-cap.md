# Same-host replace at cap — Plan

**Date:** 2026-10-23   **Status:** Draft   **Spec:** docs/toolu/specs/2026-10-23-same-host-replace-at-cap-design.md   **Topic:** regression test for same-host replace at the agent cap

## Evidence and approach
`reserve` already excludes the moving lease; add a launch-level test in the existing admission test file.

## Workstream summary
test only.

## Steps (machine-readable)
```json
[
  {
    "id": "s1",
    "title": "Same-host replace at maxAgents 1 keeps its own lease",
    "check": "cargo test -p toolu-epic-orchestrator --lib same_host_replacement",
    "ac_refs": ["AC-1"],
    "paths": ["crates/epic-orchestrator/src/launch/tests/admission_test.rs"],
    "input": "real resource home with maxAgents 1 and a herdr fixture pane"
  }
]
```

## Critical files
crates/epic-orchestrator/src/launch/tests/admission_test.rs

## Verification
The test above plus `cargo xtask gate`.
