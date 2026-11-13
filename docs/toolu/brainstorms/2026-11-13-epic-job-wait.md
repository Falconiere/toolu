# Epic job wait — Brainstorm

**Date:** 2026-11-13  **Mode:** Delivery  **Issue:** Falconiere/toolu#877

## Outcome

`toolu epic job --wait [id]` blocks for one detached result, prints the stored blocking-call output, returns its success or failure, and marks only that result delivered. With no id it selects the oldest undelivered result of the current worktree; with none it succeeds immediately.

## Evidence and choice

- Issue #877 fixes the CLI shape, the 5400-second default, no-binding requirement, timeout behavior, and three acceptance checks. Epic #869 requires each result to be told once. Merged PR #890 supplies the schema and `pending` sorted by creation time.
- Reuse the result store and resource home selected by the command environment. Resolve the current worktree without requiring its resource binding, so detached ledger jobs can be read.
- A single-result claim under the existing resource lock preserves the other pending results. Jev chose the oldest undelivered record, including a completed one, over skipping completed results or claiming them all (0.97 relative probability).

## Risks and boundaries

- A missing explicit id or a result belonging to another worktree must fail without claiming it. A timeout lists still pending ids and leaves them available.
- Keep the default blocking `job -- <argv>` path and detached launcher behavior. No new queue, watcher, or resource lease is needed for the wait itself.

## Handoff

Write the spec with real binary checks for failure output, empty pending state, timeout, and invalid command combinations.
