# Detached plan-ledger run — Design

**Date:** 2026-11-13   **Status:** Approved   **Author:** Codex   **Topic:** `toolu ledger run --detach`

## Problem

Agents wait for plan checks even when they can continue other work. Issue #876
requires an opt-in detached run that reports its outcome without stamping a
ledger for a working tree that changed while checks ran.

## Non-Goals

1. No snapshot checkout, new job queue, or second result store.
2. No change to blocking `toolu ledger run` or the shared golden cases.
3. No guarantee that the tree stays frozen after the final hash check.

## Architecture

Add `--detach` to `crates/toolu/src/ledger.rs` and a hidden `--result <id>` for
the child. The launcher creates a `waiting` result through
`toolu_engine::resources::job_results`, relaunches the native binary in the same
worktree, prints the id, and returns. Use the worktree's resource binding when
present; otherwise use `resource_home` with an empty key. The child captures
`toolu_state::tree_sha` before any check, runs the existing ledger check path
with a guarded final write, records `done` with its exit and printed text, and
calls `announce`.

For detached runs, the ledger engine compares the captured hash to a fresh
`tree_sha` immediately before its final write. A mismatch returns
`tree changed during the run` without writing the ledger; a failed hash also
refuses the write with a distinct error. Detached
`--step` defers the usual early `running` pre-write, so the same rule holds for
that form. Blocking runs retain their pre-write and output behavior.
The detached path writes its `step_run` telemetry under the result's resource
root, outside the guarded worktree. This prevents toolu's own telemetry from
making an ordinary repository without `.claude/tmp` in `.gitignore` appear
stale; check commands retain their original environment.

## Interfaces / Schema

- `toolu ledger run <DOC> [--step ID] [--activity LABEL] [--force] [--verify] [--detach]`.
- Internal child flag: `--result <id>`, paired with the original run arguments.
- The child writes schema-1 `job_results::Record` at
  `<resource root>/jobs/results/<id>.json`: `waiting` to `running` to `done`,
  with `exit_code`, `output`, `stale`, `finished_at`, and `delivered_at` as in
  merged #872. The record label identifies the ledger document and flags.
- `output` is the text the corresponding blocking run prints. A stale result
  has `stale: true`, nonzero `exit_code`, and the tree-change message.

## Failure modes and edge cases

- An invalid plan, missing Git repo, failed hash, or failed check yields a
  recorded nonzero result; a plan or hash failure writes no ledger. `stale`
  means a measured tree mismatch; hash failures remain non-stale.
- If result creation or child spawn fails, the launcher returns nonzero; a
  record already created is marked `done` with the failure where possible.
- A failed result update returns an error rather than announcing completion.
  `announce` remains best effort, as in #872.
- Both bound and unbound repositories can launch detached runs. The bound
  child still acquires the managed expensive-job lease through `ledger_run`.
- Blocking runs and existing JSON output keep their current contract.

## Acceptance criteria

- **AC-1:** In a real Git repository with a passing plan, `run --detach`
  returns an id before a sleeping check finishes; the record reaches `done`
  with exit code 0 and the ledger contains green steps.
- **AC-2:** Editing a tracked file while a detached step sleeps leaves the
  prior ledger byte-identical; the record reaches `done` with `stale: true`,
  nonzero exit, and `tree changed during the run` in `output`.
- **AC-3:** The same stale-write protection applies to `run --step ID --detach`
  despite the blocking path's early running pre-write.
- **AC-4:** Existing blocking CLI and shared ledger cases still pass, and the
  generated command contract lists `--detach`.
- **AC-5:** A bound checkout writes the result under its bound resource root
  and its check acquires the managed job lease; an ordinary checkout writes
  under `resource_home` with an empty key.

## Acceptance evidence

| AC | Real input and observable result | Check |
| --- | --- | --- |
| AC-1 | Temp Git checkout without ignore rules, approved plan with a sleeping successful check; prompt id, done/0 result, green ledger | `cargo test -p toolu-cli --test ledger_cli` |
| AC-2 | Existing ledger bytes, sleeping check, tracked edit before release; identical bytes, stale result | `cargo test -p toolu-cli --test ledger_cli` |
| AC-3 | Existing ledger bytes and `--step`, then tracked edit during check; no pre-write or final write | `cargo test -p toolu-cli --test ledger_cli` |
| AC-4 | Current blocking CLI, fixture cases and generated contract | `cargo test -p toolu-cli --test ledger_cli --test ledger_cases --test contract` |
| AC-5 | Real bound and unbound temp Git checkouts; result paths and bound lease owner observed while a check sleeps | `cargo test -p toolu-cli --test ledger_cli` |

## Documentation impact

Run `cargo xtask docs-cli` and update its generated `docs/cli/ledger.md`,
`docs/cli/commands.json`, and CLI contract snapshot. Explain detached result
location and stale behavior in the ledger user documentation.

## Open Questions

None. The issue and epic settle the interface; merged #872 and #873 provide
the result and hash contracts.
