# Epic job wait — Design

**Date:** 2026-11-13   **Status:** Approved   **Author:** Codex   **Topic:** Falconiere/toolu#877

## Problem

A detached `toolu epic job` returns an id immediately. An agent that later needs its result currently has no single blocking read, particularly for a detached ledger run.

## Non-Goals

1. No new job queue, lease, result schema, notification path, or change to the default blocking job command.
2. No agent-side polling protocol; `--wait` itself is the one blocking call.

## Architecture

Add `--wait [id]` and `--max-seconds N` to `job_command`. Dispatch the wait before the existing path that requires a resource binding and argv. Use `resource_home` from the command environment and `toolu_runtime::git::toplevel` for the current worktree. With no id, choose the first record returned by `job_results::pending`, already sorted oldest first; keep that id for the whole call. Poll the selected record internally against a monotonic deadline. Add a one-record claim under the existing resource lock. Claim only when it is done, then replay its stored output and success or failure. This uses the #872 result format without consuming other jobs.

## Interfaces / Schema

- `toolu epic job --wait [ID] [--max-seconds N]`; default `N = 5400`, matching the worker call ceiling. `--max-seconds` applies only with `--wait` and accepts a non-negative integer. `--wait` conflicts with `--detach`, `--no-wait`, hidden `--result`, and trailing command argv.
- `job_results::claim_one(root, worktree, id) -> Result<Option<Record>, String>` atomically marks just one finished, undelivered result. It never claims a record from another worktree.
- Result files and their schema remain unchanged. `output` is the text the blocking invocation printed; a nonzero `exit_code` produces a failing CLI outcome.

## Failure modes and edge cases

- No pending record and no id: exit 0 immediately with no output. An unknown, invalid, foreign-worktree, or already delivered explicit id: fail without revealing another job's output or changing its record.
- An incomplete selected record remains pending. At the deadline, fail with the current worktree's pending ids and leave them unclaimed. A zero-second limit checks once before timing out.
- A result disappearing, becoming malformed, or failing a claim/write causes a nonzero error; no success is reported. Concurrent waiters can deliver the result once at most. The loser fails if its selected record has been claimed.
- Failed detached commands return a failing outcome with the stored message, including `command exited 3`. Successful commands print the stored stdout. The wait does not acquire a managed-job lease or require a worktree resource binding.

## Acceptance criteria

- **AC-1:** Given `--detach -- sh -c 'sleep 2; exit 3'`, `--wait <id>` blocks until done, fails, prints `command exited 3`, and marks that id delivered.
- **AC-2:** Given no pending results, `--wait` returns 0 promptly; given a longer detached job, `--wait <id> --max-seconds 1` fails naming the pending id and leaves it undelivered.
- **AC-3:** `job --wait -- true` is a usage error, and the generated CLI contract lists `--wait` and `--max-seconds`.
- **AC-4:** With two undelivered results, an idless wait claims only the oldest one; a second wait can claim the other. A foreign-worktree id cannot be read.

## Acceptance evidence

| Criterion | Real input and expected observation | Check |
| --- | --- | --- |
| AC-1 | Real `sh -c` exits 3 after two seconds; wait prints its stored failure and only that result gains `delivered_at`. | `cargo test -p toolu-cli --test epic_job_detach` |
| AC-2 | Empty real Git worktree returns 0; real `sleep 2` job with one-second limit reports its id while the record remains pending. | `cargo test -p toolu-cli --test epic_job_detach` |
| AC-3 | Real CLI rejects `--wait -- true` with usage exit; generated help and JSON snapshot include both flags. | `cargo test -p toolu-cli --test epic_job_detach` and `cargo test -p toolu-cli --test contract` |
| AC-4 | Two real detached commands complete in creation order; idless waits consume one each; a second Git worktree cannot claim the first worktree's id. | `cargo test -p toolu-cli --test epic_job_detach` |

## Documentation impact

Regenerate `docs/cli/epic.md`, `docs/cli/commands.json`, and `crates/cli/tests/fixtures/commands_json.snap` with `cargo xtask docs-cli`; document the wait contract in the epic CLI help.

## Open Questions

None. Issue #877 and merged PR #890 settle the interface and result format.
