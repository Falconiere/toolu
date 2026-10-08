# Fixed GitHub checks for the epic engine — Design

**Date:** 2026-10-08   **Status:** Approved   **Author:** Codex   **Topic:** #447 GitHub change detection and fixed cadence

## Problem

The resident epic engine currently learns worker reports and Herdr events, but does not observe GitHub while a PR waits for CI, review, or merge. Worker turns still do that polling. #447 requires a fixed 180-second check for registered epics, including review threads that REST cannot detect alone.

## Non-Goals

1. Porting pr-babysit's verdict parser and fixer actions (#433), the merge queue (#448), or graph actions (#435). This issue schedules and calls the shared `BabysitTick` trait and emits detector inputs. #433 supplies the production implementation of that trait.
2. Webhooks, GitHub Apps, tunnels, public listeners, adaptive intervals, and per-PR interval settings.
3. Watching repositories or issues outside registered epics.

## Architecture

Add one GitHub watch component to `crates/epic-orchestrator`. It discovers PRs from registered issues in `babysit`, `pr-open`, and `ready` phases with a PR number; discovers epics and blockers from their registered graph files; and derives repository and base-branch watches from those entries. The state thread owns the watch table. Each watch stores an absolute `next_at_ms` and the ETags of its REST probes in `watch.json`. On startup, loading the snapshot preserves future deadlines, and an overdue watch checks once immediately. Scheduled deadlines advance by whole 180-second slots from their prior value, never from completion time. A worker report or an engine merge requests an additional immediate check without changing the scheduled deadline.

At each due PR check, conditional REST probes cover pull request, head check runs and combined status, issue comments, review comments, and reviews. A queued PR also probes its base ref. A registered epic probes its sub-issues and open blockers. The detector returns typed changes to the state thread; it never mutates issue state from a network thread. The same check invokes one full `BabysitTick` through the trait wired by `crates/cli`, which owns its GraphQL review-thread query in #433. `MergeQueue` marks the issue ready for #448; actionable and failed ticks create attention; still-running ticks leave it waiting. No probe changes does not suppress the GraphQL tick. A loopback acceptance adapter uses the real `toolu-github` client to exercise that trait boundary until #433 replaces it; production with the current `NotPorted` implementation records attention instead of claiming a green verdict.

`toolu-github::Client` supplies HTTPS, ETags, one-attempt scheduled requests, token refresh on 401, and separate REST/GraphQL `Cost` records. Use a lazily constructed client so an engine with no watches needs no token and sends no request. The existing 30-second local maintenance tick remains separate from the GitHub deadline. The socket loop wakes at the earliest of maintenance, stall, waiter, and GitHub deadlines.

Extend the shared GitHub client's rate-limit error with typed rate headers and secondary-throttle classification. The client already parses the response; the engine consumes this metadata rather than guessing from error text. Jev favored client ownership over engine inference (1.00).

## Interfaces / Schema

- One fixed `GITHUB_CHECK_MS = 180_000` constant. No setting controls it.
- `watch.json` version 1 gains `github` entries keyed by stable `owner/repo#number`, `owner/repo@base`, or epic key, each holding `nextAt`, `lastAt`, `etags`, and `retryAfterUntil`. Missing fields are initialized from the current clock. ETags never include a token.
- The detector result identifies `PrChanged`, `BaseMoved`, `EpicChanged`, `PrMerged`, or `PrClosed`, with the registered issue key. A PR tick result is a separate input and is always evaluated for a watched babysit PR.
- Journal `github-check` records identify watch key, scheduled/immediate cause, REST `304` and fresh counts, REST points, GraphQL tick count and `rateLimit.cost`, latest remaining limits, and any `retry-after` or secondary throttle. No response body or token is journaled.
- `toolu epic status --json` adds GitHub REST and GraphQL budget summaries and, for each watched PR, last and next check times. The human-readable status conveys the same fields.
- The stated GraphQL cap is 20 points per watched PR per hour: 20 scheduled checks times the observed 1-point review-thread query on `Falconiere/toolu#454` (2026-10-08). A scripted-epic budget test fails above it; the journal preserves actual cost so the cap can be reviewed against a live #433 query. The query selected `rateLimit { cost }` and `reviewThreads { nodes { isResolved } }`.

## Failure modes and edge cases

- A `429` or rate-limited `403` with `retry-after` prevents all GitHub calls for the requested duration and records the hold. The anchored 180-second schedule is not reset; the next eligible slot resumes it. A `5xx`, transport failure, or error without `retry-after` is recorded and retried at the next scheduled slot. Local engine work continues.
- If a probe lacks an ETag, it remains unconditional and its REST cost is recorded. A `304` leaves its cached identity and ETag unchanged. Head-SHA changes invalidate head-specific ETags before probing the new SHA.
- A malformed or partial reply produces an error event rather than a false state transition. A vanished or closed PR ends its watch after the terminal event. Empty registered epics and a process with no waiting PR send no GitHub request.
- A low REST or GraphQL primary budget holds new launches and merges, while checks continue. An immediate check during a retry-after hold waits for the hold; it does not move the scheduled deadline.
- The native low-budget floors preserve the documented watcher defaults: REST remaining below 1,000 or GraphQL remaining below 500. Only launches and merges wait; cleanup, checkpoints and local reporting continue. A reset deadline expires a stale low observation.
- Multi-page sub-issues, comments, and reviews must be complete before a detector concludes that nothing changed. All request paths are derived from validated `owner/repo` and numeric issue or PR IDs.

## Acceptance criteria

- **AC-1:** In a loopback HTTPS scripted epic, a verdict and green CI on a watched PR trigger a babysit tick at the next check within 180 seconds; a successful trait result enters the merge queue without a worker prompt. The production verdict parser is supplied by #433.
- **AC-2:** Over an idle scripted hour, at least 95% of conditional REST probes return `304` after warm-up; REST primary `used` stays flat on those replies, while the journal separately records every GraphQL tick and its `rateLimit.cost`, and the 20-point hourly cap is enforced.
- **AC-3:** A live 10-minute unchanged-PR watch in a sandbox repository records flat REST `used` across `304`s and reports actual GraphQL points for its scheduled ticks.
- **AC-4:** A virtual-clock six-hour wait has 180-second scheduled gaps, including after restart and immediate extra checks, with no interval backoff.
- **AC-5:** A loopback `429` with `retry-after: 60` causes no GitHub request during that 60 seconds, is journaled, and leaves subsequent scheduled gaps at 180 seconds.
- **AC-6:** An engine with no waiting PR sends zero GitHub requests during ten virtual minutes.
- **AC-7:** The engine does not create repository webhooks or open a GitHub listener; it emits change inputs for PR, base-ref, and epic graph changes from registered epics only.
- **AC-8:** `toolu epic status` shows REST and GraphQL budgets and each PR's last and next check, and the Markdown–CLI drift gate passes.

## Acceptance evidence

| AC | Real input and observable result | Runnable check |
|---|---|---|
| AC-1 | HTTPS fixture changes verdict, checks, and GraphQL thread state; a test trait adapter backed by the real GitHub client returns success, engine queues the PR at the next deadline, journal has no worker prompt. #433 repeats with the production tick | `cargo test -p toolu-epic-orchestrator github_watch` |
| AC-2 | Fixture repeats unchanged ETags for one hour and returns explicit GraphQL `rateLimit.cost`; counters and cap match journal | `cargo test -p toolu-epic-orchestrator github_budget` |
| AC-3 | Authenticated sandbox PR, four scheduled checks over ten minutes; saved live report contains both counters | documented live watch command and report in PR verification |
| AC-4 | Virtual clock over six hours, restart and immediate report injected; deadline sequence remains anchored | `cargo test -p toolu-epic-orchestrator github_cadence` |
| AC-5 | Fixture answers 429 with 60-second retry header; request timestamps and journal show hold and resumed slots | `cargo test -p toolu-epic-orchestrator github_retry_after` |
| AC-6 | Empty registered epic and no waiting PR; fixture receives zero requests | `cargo test -p toolu-epic-orchestrator github_idle` |
| AC-7 | Fixture changes PR, base ref, and sub-issue/blocker endpoints; typed events match and no webhook or listener request occurs | `cargo test -p toolu-epic-orchestrator github_detectors` |
| AC-8 | CLI status fixture and generated docs expose fields; drift gate passes | `cargo test -p toolu-epic-orchestrator status` and `cargo xtask check-markdown-cli` |

## Documentation impact

Update `docs/cli/` from the real command tree, `plugins/epic-orchestrator/README.md` for the fixed schedule and separate budgets, and engine design documentation with the watch snapshot and rate-limit behavior.

## Open Questions

None. #433 supplies the complete production babysit tick implementation through the already defined trait; #447 owns its schedule and call site. The #447 tests prove integration at the trait boundary with a real HTTPS service and the actual GitHub client; #433's own acceptance owns production verdict parsing.

## Implementation notes

The shared `TickReport` now carries optional typed GraphQL points, remaining and reset metadata. This keeps the native journal and status independent of pr-babysit's result JSON and lets #433 supply its measured query cost when ported. The current production `NotPorted` tick creates attention rather than marking a PR ready. The loopback adapter queries review threads, check rollup and review comments through the real HTTPS client to prove the schedule-to-trait boundary. The per-PR 20-point hourly assertion covers the 20 scheduled slots; an extra immediate check is counted separately.
