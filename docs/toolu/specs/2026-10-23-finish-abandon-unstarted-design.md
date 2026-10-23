# finish --abandon settles an unstarted launch — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Claude   **Topic:** `toolu epic finish --abandon` on a record whose launch died before a worktree existed (#584)

## Problem
A launch that fails before its worktree exists (for example at `gh repo clone`) leaves an issue record with no `worktree` (and maybe no `checkout`) and an `uncertain` lease. `finish` requires both fields, so `--abandon` refuses with `invalid issue record …: checkout`, then `worktree`. `--replace` needs a live agent and `--reprompt` starts a new worker, so the lease holds a slot forever.

## Non-Goals
1. No new command or flag.
2. No change to a record that has a worktree: it keeps the strict snapshot/remove teardown.
3. No change to finish without `--abandon`: it still needs a merged PR.
4. No lease reclaim by liveness (that is #583).

## Architecture
A record is *unstarted* when it has no `worktree`, `workspace_id`, `agent`, `pane_id`, `session_id` or `removal_intent`. `read_issue_record` then does not require `checkout`/`worktree` (a present `checkout` must still be absolute); a record that has a `worktree` still needs both. `finish` with `--abandon` on an unstarted record skips the teardown: nothing exists to snapshot, exit or remove. It settles through the existing `settle`/`record_finish` path with `Removal { removed: true, branch_deleted: false, wip_ref: None, dirty: [] }`, so the record becomes `abandoned` with `worktree_removed: true`, and `release_recorded_ownership` releases the lease. A repeat run is already idempotent through `terminal_cleanup_complete`. Reuses `finish/record.rs`, `finish/ownership.rs`.

## Interfaces / Schema
`record::never_started(&Record) -> bool` (crate-private). No JSON schema change; the settled record gains no new field. Dry run prints one action, `release lease <token>` (or no action when the record owns none), and no snapshot/remove lines.

## Failure modes and edge cases
- Worktree path recorded but missing on disk, or `workspace_id`/`pane_id` set: not unstarted, strict path unchanged.
- `worktree` set without `checkout`: still `invalid issue record …: checkout`.
- Lease token recorded but owned by another issue: refused with "recorded agent resource ownership does not match issue", record left as written.
- Lease already gone: finish succeeds (existing behavior).
- Non-`--abandon` on an unstarted record: refused "no PR recorded", nothing written.
- Terminal `abandoned` record re-run: releases again, succeeds.

## Acceptance criteria
- **AC-1:** Given an `uncertain` record with no `checkout`/`worktree` and an agent lease, `finish --abandon` returns stage `abandoned`, the record is `abandoned` with `worktree_removed: true`, and the lease is gone.
- **AC-2:** Given a record with `checkout` but no `worktree`, `finish --abandon` behaves as AC-1.
- **AC-3:** `finish --abandon --dry-run` on the AC-1 record lists a lease release and no snapshot/remove, and writes nothing.
- **AC-4:** Without `--abandon`, the AC-1 record is refused and unchanged; a record with `worktree` but no `checkout` is still refused with `checkout`.
- **AC-5:** Running the AC-1 abandon twice succeeds both times and leaves the record `abandoned`.

## Acceptance evidence
All use the real finish rig (`tests/finish_rig_test.rs`: temp state dir, real resource store, real lease) in `tests/finish_test.rs` and `tests/record_test.rs`; run `cargo test -p toolu-epic-orchestrator finish`. AC-1/2/5: record JSON and `leases()` after the call. AC-3: report actions and unchanged record bytes. AC-4: exact error strings and unchanged bytes.

## Documentation impact
None — existing `--abandon` flag, `docs/cli/` unchanged; the module doc of `finish.rs` is updated.

## Open Questions
None blocking. Decided: a `checkout`-only record counts as unstarted (the issue shows it refusing next on `worktree`).
