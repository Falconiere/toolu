# Stop with an unread detached job — Design

**Date:** 2026-11-13  **Status:** Approved  **Author:** Codex  **Topic:** Falconiere/toolu#880

## Problem

A worker can end its turn while a detached `toolu` job is still running or its result has not been read. The worker needs one clear chance to account for that job.

## Non-Goals

1. No new job queue, polling loop, or change to blocking job commands.
2. No Stop registration on hosts without a verified blocking Stop contract.
3. No change to the existing completion claim semantics.

## Architecture

Add `HostEvent::Stop` and native mappings for Claude and Muse, whose installed runners were probed live. Register a generated `stop` hook in those `toolu` manifests. The CLI handler resolves the worktree and existing resource root, reads the first undelivered job result, and writes a per-id Stop marker under the resource lock. On the first encounter it exits 2 with a reason naming the id; a repeat passes. `delivered_at` remains available to the normal completion path. A result already marked delivered never blocks.

## Interfaces / Schema

- `HostEvent::Stop` has slug `stop`; `native_event` returns `Stop` for Claude and Muse and `None` elsewhere.
- The job result schema remains 1. A Stop marker is `<resource root>/jobs/results/stop-blocked/<id>`; it contains no result data and is written atomically under the resource lock.
- `job_results::block_stop_once(root, worktree) -> Result<Option<Record>, String>` selects the oldest undelivered, unwarned result and writes its marker under the shared resource lock.
- `toolu hook stop --event Stop` uses the existing hook entry point and exits 2 with a stderr reason naming the job id. No result produces silent success.

## Failure modes and edge cases

An absent result directory passes. A result in another worktree or already delivered does not block. A pending result remains eligible for its later completion announcement. Two concurrent Stop calls cannot both mark the same result. An unreadable or invalid result record is reported as a hook failure with a reason; it is not silently ignored. A Stop with no usable worktree or resource root reports that failure instead of guessing. The second Stop for the same record passes; a different unread result can block once too.

## Acceptance criteria

- **AC-1:** With a real pending result for the current worktree, the first Stop blocks and names its id; the second Stop passes.
- **AC-2:** A completed, unclaimed result blocks once, while a delivered result and a result for another worktree pass.
- **AC-3:** After a pending-result Stop block, the completed result remains claimable by the normal delivery path.
- **AC-4:** Generated hook entries match the manifests, and the `toolu/stop` hook meets the hook latency assertion.
- **AC-5:** The PR records which installed hosts were probed live and their Stop verdicts.

## Acceptance evidence

- AC-1: Create a real schema-1 waiting result with the engine API in a temporary worktree; invoke the built `toolu hook stop --event Stop` twice. Check `cargo test -p toolu-cli --test tool_hooks`.
- AC-2: Use real result files in two temporary worktrees, one completed and one claimed; invoke the same binary. Check `cargo test -p toolu-cli --test tool_hooks`.
- AC-3: Complete the warned record and call `claim_finished`; it must return the id once. Check `cargo test -p toolu-engine --lib -- resources::`.
- AC-4: Check `cargo xtask check-hooks` and `cargo xtask bench-hooks --only toolu/stop --assert`.
- AC-5: Claude Code 2.1.323, with isolated Stop settings and a first-exit-2/second-exit-0 hook, made two turns and read the reason. Muse Code 1.4.20, with an isolated trusted native plugin and Stop fixture, returned `should_block: true`, `status: blocked`, exit 2, and the reason. Repeat on the generated hook and record both in the PR.

## Documentation impact

Update the toolu plugin hook documentation and CLI hook contract inventory for the new event. Record supported hosts and the once-only behavior.

## Open Questions

None. Claude and Muse passed the live Stop probes; other hosts remain outside the registration scope.
