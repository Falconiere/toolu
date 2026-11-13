# Detached plan-ledger run

**Date:** 2026-11-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-13-detached-ledger-run-design.md   **Topic:** `toolu ledger run --detach`

## Evidence and approach

Issue #876 and epic #869 settle the opt-in CLI and result contract. Merged
#872 supplies `job_results` and native orphan relaunch; merged #873 supplies
`tree_sha`. `ledger/run.rs` writes twice for `--step`, so the detached path
must defer its early write and guard the final write. The blocking path and
shared fixture cases remain authoritative.

## Workstream summary

Add the guarded engine path and native detached CLI, prove it with real Git
checkouts, regenerate CLI docs, then run changed checks and the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Guard detached ledger writes and run the native child",
    "input": "Real temporary Git repositories without ignore rules, a sleeping plan check, a tracked edit, and bound and unbound resource roots",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-5"],
    "check": "cargo test -p toolu-cli --test ledger_cli"
  },
  {
    "id": "s2",
    "title": "Preserve blocking ledger cases and generate the CLI contract",
    "depends_on": ["s1"],
    "input": "Existing fixtures/ledger/cases.json and the generated native command tree",
    "ac_refs": ["AC-4"],
    "check": "cargo xtask docs-cli --check && cargo test -p toolu-cli --test ledger_cases --test contract && cargo xtask check-markdown-cli"
  },
  {
    "id": "s3",
    "title": "Run checks for changed crates and dependents",
    "depends_on": ["s1", "s2"],
    "input": "Branch diff against origin/main",
    "check": "cargo xtask check-changed --base origin/main"
  },
  {
    "id": "s4",
    "title": "Pass the complete quality gate",
    "depends_on": ["s3"],
    "input": "Complete branch including generated docs and real-data tests",
    "check": "cargo xtask gate"
  }
]
```

## Critical files

- `crates/toolu/src/ledger.rs`, `crates/toolu/src/ledger_detach.rs` and sibling unit tests.
- `crates/core/engine/src/ledger/run.rs` and sibling unit tests.
- `crates/core/engine/src/ledger/context.rs` and `step.rs` for telemetry
  placement during a guarded run.
- `crates/cli/tests/ledger_cli.rs` and its existing real Git helper.
- `docs/cli/ledger.md`, `docs/cli/commands.json`, and
  `crates/cli/tests/fixtures/commands_json.snap` from `cargo xtask docs-cli`.

## Verification

Use a sleeping check with a file marker to prove the launcher returns before
completion, then observe the result and ledger. Edit a tracked file before the
check ends for both full and `--step` forms and compare ledger bytes. Verify
bound result placement and lease ownership, plus the unbound fallback. Run
`cargo xtask docs-cli` before `s2`, then re-run unchanged blocking cases and
the generated contract. The behavior steps omit `paths` because they read
multiple crates and fixtures and must be rechecked on every ledger run. After
the scoped commit, run `toolu ledger run <this plan> --verify`, the complete
version-2 review, `toolu ledger verdict status`, then push and open the PR
only when all gates are ready. The plan's pathless `s4` reruns the full gate
during final verification.

## Deviations

The first real passing test produced `stale: true` because toolu appended
telemetry inside a checkout with no `.gitignore`. `s1` now routes only detached
telemetry to the result resource root; the test exercises this boundary.
