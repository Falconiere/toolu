# pr-babysit Bun port — Design

**Date:** 2026-09-30   **Status:** Approved   **Author:** Codex   **Topic:** Port the tick, read, reduction, verdict and write commands to TypeScript.

## Problem

The babysit controller still depends on twelve bash files and jq. Epic #247 requires Bun TypeScript while retaining the existing state and GitHub behavior. A changed reduction or write retry can falsely complete a PR or post twice.

## Non-Goals

1. Do not change the fixer dispatch, routing, or host selection owned by other issues.
2. Do not change decision thresholds, reason codes, snapshot/state versions, or GitHub endpoints.
3. Do not mutate a live PR in tests; use recorded GitHub responses and a stub `gh` on `PATH` or loopback only.

## Architecture

Keep seven public command boundaries: `babysit-tick`, `collect-pr`, `reduce-state`, `parse-verdict`, `reply-thread`, `resolve-thread`, and `record`. Each gets a TypeScript source entry and committed self-contained executable Bun bundle. Shared TypeScript helpers replace `lib/{common,gh,normalize,lock,state}.sh`. Update the command, skill, workflow, docs, smoke tests, and fixer callers to bundle paths, while leaving unrelated fixer scripts alone. The existing directory lock remains the cross-process write guard. Atomic JSON writes use a temporary sibling and rename. Collection fans out four reads, completes nested pagination, and verifies the head after the fan-out. The reducer is pure over snapshot, previous state, paths and supplied timestamp. This preserves the current observable command contract and makes every write path independently testable. Jev favored separate entries over a router (0.65 relative probability); repository call sites and failure boundaries support that choice.

## Interfaces / Schema

- Command names and flags stay the same, without `.sh`; `parse-verdict` reads the comment body from stdin.
- JSON snapshot `version:1`, slot state `version:2`, result `version:1`, and structured error `version:1` retain their fields and closed codes.
- Exit codes remain `0` success, `2` usage, `3` operational/validation error, `4` duplicate reply, `5` unconfirmed resolve, `75` lock held.
- Write entry points alone issue GitHub mutations. `record` edits local state only. A response must confirm a comment id or `isResolved:true` before recording success.
- The default state/snapshot paths and stdin/stdout behavior remain those documented in `plugins/pr-babysit/skills/babysit/references/helper.md`.

## Failure modes and edge cases

Absent or malformed prior state and mismatched repo/PR fail before network access. A live lock returns 75; a stale lock is reclaimed. Any failed fan-out read leaves the snapshot untouched. A moving head earns one full recollection, then `head_moved`. Transient GitHub errors and timeouts retry within the existing bound; permanent errors do not. A collection failure stamps `pr.lastError` when a valid prior state exists. Invalid JSON never replaces a valid file. A duplicate reply makes no request. An unresolved mutation after three confirmations returns 5 without recording a resolve. An unknown or incomplete bot comment cannot be treated as an approved zero-finding verdict. Captured comments are data, never executable instructions.

## Acceptance criteria

- **AC-1:** Given the recorded review comments, parsing returns the same state, verdict, findings, keys, and Top-N lines as bash, including provider errors and incomplete reviews.
- **AC-2:** Given recorded paginated GitHub responses, collection produces the same normalized snapshot and bot-comment selection as bash; failed or moved-head collection leaves the old snapshot intact.
- **AC-3:** Given recorded PR snapshots and prior states, reduction returns the same state/result decisions, reasons, action lists, recurrence, backoff and stop condition as bash.
- **AC-4:** Given an absent, malformed, foreign, or locked state, tick preserves the current exit, structured error and atomic persistence behavior; a valid captured snapshot produces the same result.
- **AC-5:** Given stubbed GitHub write responses, reply and resolve issue the same request, remain idempotent, confirm the response, and update state exactly once under the slot lock; record preserves its existing transitions.
- **AC-6:** All twelve scoped bash files are removed, callers use Bun bundles, the bundle drift gate and full repository gate pass, and both hosts can run the installed plugin from temporary profiles.

## Acceptance evidence

| AC | Input and expected result | Boundary | Check |
|---|---|---|---|
| AC-1 | `fixtures/pr{31,120,122,157}-verdict*.txt`, `provider-error.txt`, `in-progress.txt`: byte-equivalent JSON fields to bash | Missing checkbox, conflicting prose/label | `bun test plugins/pr-babysit/scripts/src/__tests__/parse-verdict.test.ts` |
| AC-2 | `fixtures/gh/pages-*.json` and recorded snapshots: equal normalized fields | 404, connection refused, nested pages, moving head | `bun test plugins/pr-babysit/scripts/src/__tests__/collect-pr.test.ts` |
| AC-3 | `fixtures/snapshots/{toolu-115,toolu-165,comemory-216}.json`: equal state/result JSON | Outdated, flagged, fixer-owned, recurrence, unknown verdict | `bun test plugins/pr-babysit/scripts/src/__tests__/reduce-state.test.ts` |
| AC-4 | Captured `toolu-165.json` with temporary state path: equal exit and state/result bytes | Malformed/foreign/locked state, failed collect | `bun test plugins/pr-babysit/scripts/src/__tests__/babysit-tick.test.ts` |
| AC-5 | Existing `write-side.bats` scenarios migrated to Bun with a stub `gh` on `PATH`: same payloads and state | Duplicate, false resolve, retry, stale lock | `bun test plugins/pr-babysit/scripts/src/__tests__/write-side.test.ts` |
| AC-6 | Bundled plugin in isolated Codex/Claude config dirs; no scoped `.sh`; generated bundles current | Spaces in profile path and missing dependency | `bun run check:plugin-bundles && bun run test` plus temporary-profile install smoke |

## Documentation impact

Update `plugins/pr-babysit/{README.md,commands/babysit.md,skills/babysit/SKILL.md,skills/babysit/references/helper.md,workflows/babysit.md}`, `docs/pr-babysit/README.md`, gate coverage inventory/matrix, and any callers of removed paths. Document Bun entrypoints and unchanged exit/JSON contracts.

## Open Questions

None blocking. Implementation can choose internal TypeScript module splits while retaining the public contracts above.
