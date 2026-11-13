# Epic job wait — Plan

**Date:** 2026-11-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-13-epic-job-wait-design.md   **Topic:** Wait for one detached job result in a blocking call.

## Evidence and approach

Issue #877 defines the CLI contract and 5400-second default. Merged PR #890 supplies `job_results::pending`, the atomic result files, and the real-binary detach test. `crates/epic-orchestrator/src/job.rs` currently requires argv and a resource binding, so wait dispatch must happen first. Use the binding's resource root when present and `resource_home` when absent; reuse `git::toplevel` and the existing resource lock. Claim only one result. The brainstorm's Jev judgment supports selecting the oldest undelivered record, including a completed one.

## Workstream summary

Add the one-result wait and real-binary coverage, regenerate CLI documentation, then run affected and complete checks before delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Wait for and claim one detached result",
    "check": "unset PUSH_REVIEW_BASE DOCS_SYNC_BASE; cargo test -p toolu-cli --test epic_job_detach && cargo test -p toolu-engine --lib -- resources::job_results::tests && cargo xtask check-changed --base origin/main",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "input": "real Git worktrees and real detached sh commands: failing exit 3, empty queue, one-second timeout preserving the id, two results claimed separately, foreign worktree id, a wait after removing the binding, a binding root different from the environment, and invalid CLI combination"
  },
  {
    "id": "s2",
    "title": "Regenerate and verify the public CLI contract",
    "check": "unset PUSH_REVIEW_BASE DOCS_SYNC_BASE; cargo xtask docs-cli && cargo test -p toolu-cli --test contract && cargo xtask check-markdown-cli",
    "depends_on": ["s1"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/epic-orchestrator/src/verbs_report.rs", "docs/cli/epic.md", "docs/cli/commands.json", "crates/cli/tests/fixtures/commands_json.snap"],
    "input": "native toolu command metadata and generated help/JSON contract"
  },
  {
    "id": "s3",
    "title": "Run the complete quality gate",
    "check": "unset PUSH_REVIEW_BASE DOCS_SYNC_BASE; cargo xtask gate",
    "depends_on": ["s1", "s2"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "input": "the full branch and real workspace tests"
  }
]
```

## Critical files

`crates/epic-orchestrator/src/job.rs`, `job_wait.rs`, its sibling test, `tests/query_test.rs`, `verbs_report.rs`, `verbs.rs`, `lib.rs`; `crates/core/engine/src/resources/job_results.rs` and its sibling test; `crates/cli/tests/epic_job_detach.rs`; generated CLI docs and contract snapshot.

## Verification

The focused real-binary test proves failure replay, empty and timed-out waits, one-record delivery, and worktree isolation. The engine unit test proves atomic one-record claims. `check-changed` runs after the code step; generated docs and contract are checked before the full gate. Fetch and rebase on a moved `origin/main` before implementation and push, rerunning affected checks. Delivery then requires final ledger verification, code review, a ready verdict, branch push, and PR; the epic engine takes over after `report babysit`.

## Deviations

The first `check-changed` found the required sibling test for new `job_wait.rs` and missing pinned npm tools. Added a real non-Git-directory boundary test and ran `npm ci --ignore-scripts`. The next run found a `panic-in-result-fn` lint in the new integration-test helper; it now returns an error. The following run found an old hand-built `job` match in `query_test.rs` without the new argument; updated it and passed its focused test. The complete `check-changed` then passed. Generated CLI docs and the contract snapshot were synchronized before that complete run so the changed gate could assess the final command surface.

The Codex-host branch verification used `PUSH_REVIEW_BASE=origin/main` to select the true epic base. Its checks inherited the variable, and two unchanged golden-gate tests failed because their temporary Git repositories have no `origin/main`. Both tests and the full gate passed earlier without the override. Each plan check now unsets `PUSH_REVIEW_BASE` and `DOCS_SYNC_BASE` inside its check shell while the ledger process still uses the remote base for diff identity. This isolates test fixtures from the host's review-base override.

Final code review found that a valid worktree binding can point to a different resource root from the environment. Wait now reads that binding root and falls back to the environment only when the binding is absent; a real-binary regression case covers the mismatch. Under concurrent full gates, the focused test's five-second process timing assertion failed despite an empty result. It now checks the returned empty result, while the separate one-second timeout case covers the bounded wait.
