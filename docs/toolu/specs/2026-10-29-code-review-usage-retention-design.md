# Code-review usage retention — Design

**Date:** 2026-10-29   **Status:** Approved   **Author:** Codex (epic worker, Falconiere/toolu#750)   **Topic:** Bound project review usage JSONL

## Problem

Each review appends a usage row to project state without any retention limit. Long-lived projects accumulate an unbounded log, while review accounting must still fail closed if a row cannot be recorded.

## Non-Goals

1. No change to review usage fields, billing calculation, or push-review state format.
2. No retention change to ast-grep byte-savings logs (#749).
3. No new user configuration or usage-reader command.

## Architecture

Keep `code-review-usage.jsonl` as the active append-only file. Serialize each row and reject one over 1 MiB before acquiring `with_lock_strict`. Under the lock, rotate before the append would exceed 1 MiB. Move a full active file to `code-review-usage/<sequence>.jsonl`; after appending, reclaim the oldest archives under the same lock to keep the four newest complete archives. An oversized pre-policy active file is reclaimed at its first rotation because it cannot fit a bounded generation. Preserve `persist`'s existing error-to-blocker path.

Use a stale lock interval long enough for a live writer and a five-second acquisition timeout; a busy or unavailable lock fails the write. The exact active file and lock, plus 20-digit numbered archive `.jsonl` files, are the only new dirty-probe exclusions. An unrelated file in the archive directory remains dirty.

## Interfaces / Schema

`log_usage(&Roots, &Path, &str, &Report) -> Result<(), String>` and the JSON row stay unchanged. Active path: `<project-state>/code-review-usage.jsonl`. Archive paths: `<project-state>/code-review-usage/<20-digit-sequence>.jsonl`, oldest sequence first. Lock path: `<project-state>/code-review-usage.jsonl.lock`. At rest after a successful write, retained storage has at most five files of at most 1 MiB each.

## Failure modes and edge cases

- Missing directories are created. Unreadable metadata, failed lock, failed archive cleanup, failed rotation, and failed open/write propagate as useful errors; `persist` writes a blocking gate state.
- A short or failed append is rolled back to its previous length. If rollback also fails, return both errors and leave the review blocked.
- An absent or empty active file does not rotate. A row exactly at the size boundary fits. An oversized row fails before touching the log.
- A crash after moving the active file but before opening the new one leaves complete archives; the next writer creates the active file. Cleanup and concurrent writers use the same lock.
- Existing oversized active files are reclaimed when they first need rotation; their former records are beyond the new retention window.

## Acceptance criteria

- **AC-1:** Repeated real usage writes crossing 1 MiB create only complete JSONL rows in the active file and at most four numbered archives; retained size is at most 5 MiB after success.
- **AC-2:** An append, rotation, cleanup, or lock failure returns an error and cannot write passing push-review state; a short write cannot leave a partial retained row when rollback succeeds.
- **AC-3:** Concurrent local writers each return success only after their row appears exactly once in retained output, provided the test does not exceed the retention window.
- **AC-4:** In a temporary Git worktree, active usage, archived JSONL, and lock artifacts do not mark the review dirty; an unrelated file, including one in the archive directory, still does.
- **AC-5:** The final pushed head passes focused crate tests, ledger verification, and `cargo xtask gate`; two independent `toolu-review:review` passes report clean findings and write review state.

## Acceptance evidence

| AC | Real input | Expected observation | Boundary | Runnable check |
| --- | --- | --- | --- | --- |
| AC-1 | Repeated 1 MiB-scale JSONL rows in a temp project state root | Valid JSONL, at most four archives plus active, <=5 MiB | Exact limit and oversized legacy file | `cargo test -p toolu-code-review usage_retention` |
| AC-2 | Temp file path made unwritable or occupied by a directory; held lock | Error; no passing gate state; no incomplete row after recoverable write failure | Denied rotation/append and busy lock | `cargo test -p toolu-code-review usage_failure` |
| AC-3 | Parallel real filesystem appends through `log_usage` | Each successful row occurs once | Shared archive transition | `cargo test -p toolu-code-review usage_concurrent` |
| AC-4 | Real temporary Git repository with usage artifacts and `notes.jsonl` | Own artifacts clean, unrelated JSONL dirty | Lock present and unrelated JSONL in archive directory | `cargo test -p toolu-code-review only_untracked_plugin_state_is_ignored` |
| AC-5 | Final committed and pushed branch diff | Green ledger and gate; two clean review passes | All changed files | `toolu ledger run docs/toolu/plans/2026-10-29-code-review-usage-retention.md --verify`; `toolu epic job -- cargo xtask gate`; two skill reviews |

## Documentation impact

Update `plugins/code-review/README.md` and `docs/config.md` with the retention limit, archive ordering, quiescent reading, and reclamation rule.

## Open Questions

None. The issue permits the worker to choose the policy; the finite size and generation count above define the behavior.

## Spec review (2026-10-29)

No blocking findings. The contract names the exact retained paths and limits, preserves the existing failure-to-blocker path, and ties each issue outcome to a real-file or real-Git check. The test plan must exercise both the log error and its propagation through `persist`; Jev judged the contract aligned with the issue (0.78).
