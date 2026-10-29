# Code-review usage retention — Plan

**Date:** 2026-10-29   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-29-code-review-usage-retention-design.md   **Topic:** Bounded project review usage JSONL (#750)

## Evidence and approach

`pipeline/state.rs::log_usage` has one append caller through `persist`. The existing `persist` error path adds a blocking state finding. `shape/dirty.rs` excludes only the active log today, and its sibling test uses a real temporary Git repository. Reuse `toolu_state::lock::with_lock_strict` with a long stale interval. Rotate complete files by size into a numbered archive directory; preserve fail-closed writes and test the exact dirty exclusions.

## Workstream summary

Reproduce the unbounded log, implement locked retention and failures, extend the dirty probe, document the policy, measure release builds, and run the committed-diff gate and reviews.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"Locked bounded JSONL append and real-file regression","check":"cargo test -p toolu-code-review usage_","ac_refs":["AC-1","AC-2","AC-3"],"paths":["crates/code-review/src/pipeline/state.rs","crates/code-review/src/pipeline/tests/state_test.rs","crates/code-review/src/tests/pipeline_test.rs","crates/code-review/src/shape/dirty.rs","crates/code-review/src/contract.rs","crates/core/state/src/lock.rs"],"input":"Real temporary state roots; enough review usage rows to cross five 1 MiB generations; oversized row and legacy file; occupied path and held lock; parallel writes with distinct review hashes; parse every retained line and check counts and error propagation."},
  {"id":"s2","title":"Narrow dirty-tree exclusions and usage documentation","check":"cargo test -p toolu-code-review only_untracked_plugin_state_is_ignored && cargo xtask check-markdown-cli","ac_refs":["AC-4"],"depends_on":["s1"],"paths":["crates/code-review/src/shape/dirty.rs","crates/code-review/src/shape/tests/dirty_test.rs","plugins/code-review/README.md","docs/config.md"],"input":"Real temporary Git worktree containing the active log, numbered archive JSONL, held lock file, and unrelated notes.jsonl in the archive directory; documented archive/read and reclamation paths."},
  {"id":"s3","title":"Release-build baseline and retained-resource evidence","check":"cargo test --release -p toolu-code-review usage_retention_release_measurement -- --nocapture","ac_refs":["AC-1"],"depends_on":["s1"],"paths":["crates/code-review/src/pipeline/state.rs","crates/code-review/src/pipeline/tests/state_test.rs","docs/toolu/evidence/2026-10-29-code-review-usage-retention.md"],"input":"Repeat a fixed workload of local usage rows on this platform with the baseline and changed release builds; record sample count, runtime, retained bytes/files, and open descriptor and thread counts."},
  {"id":"s4","title":"Final committed-diff gate","check":"cargo xtask gate","ac_refs":["AC-5"],"depends_on":["s1","s2","s3"],"paths":["crates/code-review","plugins/code-review/README.md","docs/config.md","docs/toolu/brainstorms/2026-10-29-code-review-usage-retention.md","docs/toolu/specs/2026-10-29-code-review-usage-retention-design.md","docs/toolu/plans/2026-10-29-code-review-usage-retention.md","docs/toolu/evidence/2026-10-29-code-review-usage-retention.md"],"input":"Final committed branch diff with real-file tests, docs, measurement, and all repository quality checks."}
]
```

## Critical files

Modify `crates/code-review/src/pipeline/state.rs`, its sibling `tests/state_test.rs`, `crates/code-review/src/tests/pipeline_test.rs`, `crates/code-review/src/shape/dirty.rs`, its sibling `tests/dirty_test.rs`, `plugins/code-review/README.md`, and `docs/config.md`. Add this plan, the spec, brainstorm capsule, and `docs/toolu/evidence/2026-10-29-code-review-usage-retention.md`. No CLI, fixture, or dependency change is needed.

## Verification

First observe the baseline growth with a release test, then make s1 and s2 green. Record before/after release measurements with the same workload. Run expensive checks through `toolu epic job --`. Commit scoped changes, verify every ledger step against the final diff, run the full gate, rebase onto `origin/main` if it moved and repeat affected checks, then push. On pushed HEAD, run two independent `toolu-review:review` passes and write v2 review state after each clean pass. Confirm `toolu ledger verdict status` is ready, create or verify the PR, and report `ready` for orchestrator admin merge. Do not start babysit or merge.

## Plan review (2026-10-29)

No blocking findings. All five spec criteria have ledger coverage and real-input checks. The pushed-head reviews remain an explicit delivery check after the branch is pushed; they cannot be a ledger step without making plan readiness depend on itself. `pl_check_ac_refs` is not installed in this host, so `toolu ledger preflight` and status provide the machine check. Jev returned 0.4 on full alignment, reflecting that split; the review requirement is retained in this verification section and must be observed before `ready`.

## Deviations

The fail-closed integration check extended the existing `pipeline_test.rs` case, and the real rotation check also calls `shape::dirty_except_state`. Both paths were added to s1's freshness declaration before stamping it.

A pre-commit review found the whole-directory dirty exclusion could hide unrelated files. The s2 input and approved spec now require the archived JSONL glob and assert that `note.txt` in the archive directory remains dirty.

Review pass 1 found that `*.jsonl` still hid unrelated `notes.jsonl`. The exclusion now matches the writer's exact 20-digit archive name shape; s2 uses a real Git `notes.jsonl` to prove the narrower boundary.
