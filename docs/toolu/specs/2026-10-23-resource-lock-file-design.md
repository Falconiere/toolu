# Resource lock file diagnostic — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Codex   **Topic:** A regular file at `state.lock` blocks resource leases.

## Problem

An external `flock` command can create a regular file where the resource lock directory belongs. Every lease operation then reports `state.lock/owner.json: Not a directory`, without identifying the offending file or a recovery action.

## Non-Goals

1. Automatically delete or replace a lock file whose provenance is unknown.
2. Change lease admission, owner liveness, or `.reap` recovery policy.

## Architecture

At lock acquisition, inspect an existing lock path before treating it as a directory. Return an actionable error if it is a regular file. Preserve it for operator inspection. Keep the directory owner protocol and stale owner recovery intact. The error names the lock path and instructs the operator to confirm that no resource update is using it, then remove the file.

## Interfaces / Schema

`acquire_lock(path: &Path, timeout: Duration) -> Result<Option<LockHandle>, String>` and `with_resource_lock(root, f)` retain their signatures. The error text includes the offending `path`, says it is not a directory, and describes safe manual removal. No state schema changes.

## Failure modes and edge cases

- Absent path: create a normal directory lock.
- Held directory: wait or report busy as before.
- Empty or nonempty regular file: return the same actionable error without modifying the file or invoking the closure.
- Unexpected file system metadata error: propagate with the lock path.
- Concurrent path replacement: repeat the type check before reading `owner.json`; an unexpected race can still yield an operating system error, but cannot trigger automatic file deletion.
- Existing `.reap`: keep the existing fail-closed behavior for a directory lock; a regular file still gets the actionable diagnostic.

## Acceptance criteria

- **AC-1:** Given a regular file at `state.lock`, acquisition and a resource update report the offending path and a manual recovery action, leaving file bytes intact.
- **AC-2:** Given an absent, held, or stale-owner lock directory, acquisition keeps the current exclusive, busy, and dead-owner recovery behavior.

## Acceptance evidence

- AC-1: A real temporary resource home with a nonempty `state.lock` file; run the focused lock tests and assert exact path, diagnostic, preserved bytes, and that the update closure did not run.
- AC-2: Existing real-directory lock tests, run through `cargo test -p toolu-engine resources::lock::tests`; assert exclusive acquisition, busy timeout, token-safe release, and dead-owner recovery.

## Documentation impact

Update `docs/epic-orchestrator/README.md` with the diagnostic and recovery action.

## Open Questions

None. Issue #590 explicitly permits an actionable diagnostic. The conservative choice matches the existing fail-closed owner protocol.
