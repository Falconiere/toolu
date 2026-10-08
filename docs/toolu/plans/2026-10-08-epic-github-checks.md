# Fixed GitHub checks for the epic engine — Plan

**Date:** 2026-10-08   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-08-epic-github-checks-design.md   **Topic:** #447. Conditional GitHub change detectors and a fixed 180-second engine schedule.

## Evidence and approach

The reviewed spec decides the boundaries. `crates/epic-orchestrator/src/socket.rs` has a 30-second maintenance tick; `model.rs` and `watch.json` hold persisted deadlines; `crates/core/github` supplies conditional GETs, GraphQL, and successful-call costs. `BabysitTick` is wired through `crates/cli`; its production implementation is #433. The issue and #402 require 180-second checks with no backoff, a full GraphQL tick per babysit check, separate REST and GraphQL budgets, and no webhook. A live GraphQL review-thread query against `Falconiere/toolu#454` reported `rateLimit.cost=1` on 2026-10-08. Comemory `4a085d57` and `f8e84ece` identify the shared-job lease syntax and rustup PATH needed for gates.

## Workstream summary

Build the persisted clock and watch discovery, then the HTTPS detectors and rate metadata, then connect the scheduler to the resident engine and status. Verify with virtual time and loopback HTTPS, perform the required live ten-minute watch, update docs, and run the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "watch-clock",
    "title": "Discover registered PR, base and epic watches; persist ETags and anchored 180-second deadlines in watch.json; add virtual-clock restart, immediate-check, idle and six-hour cadence cases",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator github_",
    "ac_refs": ["AC-4", "AC-6"],
    "paths": ["crates/epic-orchestrator/"],
    "input": "registered graph and status files from an isolated temp epic; virtual timestamps spanning 6 hours, a restart and an immediate worker report"
  },
  {
    "id": "detectors",
    "title": "Implement conditional REST detectors through toolu-github, complete pagination and head-SHA invalidation; expose typed rate metadata on errors; produce typed PR, base and epic change inputs",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator github_detectors && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-github rate_limit",
    "ac_refs": ["AC-2", "AC-5", "AC-7"],
    "depends_on": ["watch-clock"],
    "paths": ["crates/epic-orchestrator/", "crates/core/github/", "Cargo.lock"],
    "input": "real loopback HTTPS fixture: ETag 200 then 304, changed head SHA, paginated reviews/sub-issues, 429 retry-after 60, secondary 403 and 5xx"
  },
  {
    "id": "engine-check",
    "title": "Wake the state thread for GitHub deadlines; probe watches, invoke BabysitTick every babysit slot, journal separate costs and throttles, hold launches and merges under low budgets, and expose last/next check in status",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator github_watch && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator github_budget && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator github_retry_after && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-epic-orchestrator status",
    "ac_refs": ["AC-1", "AC-2", "AC-4", "AC-5", "AC-8"],
    "depends_on": ["detectors"],
    "paths": ["crates/epic-orchestrator/", "crates/cli/", "crates/core/github/", "Cargo.lock"],
    "input": "loopback HTTPS scripted epic with changing bot verdict, green CI, GraphQL threads and rateLimit.cost; adapter implements the real BabysitTick boundary using toolu-github; journal and status are read from disk"
  },
  {
    "id": "live-and-docs",
    "title": "Observe an unchanged PR for ten minutes, record paired REST 304 primary counters and GraphQL points, document the fixed schedule, budget and pending #433 verdict boundary, and regenerate CLI reference",
    "check": "jq -e '(.endedAt | fromdateiso8601) - (.startedAt | fromdateiso8601) >= 600 and (.checks | length >= 4) and ([.checks[] | .graphqlCost] | all(. > 0)) and ([.checks[1:][] | .restNotModified] | all(. >= 1)) and ([.checks[1:][] | select(.restNotModified >= 2)] | length >= 2) and ([.checks[1:][] | .restUsedDelta] | all(. == 0))' docs/toolu/evidence/2026-10-08-github-live-watch.json && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-markdown-cli && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only docs-cli && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'PATH=/root/.cargo/bin:/root/.bun/bin:$PATH /root/.bun/bin/bun run test:docs'",
    "ac_refs": ["AC-3", "AC-7", "AC-8"],
    "depends_on": ["engine-check"],
    "paths": ["crates/epic-orchestrator/", "docs/cli/", "docs/toolu/", "plugins/epic-orchestrator/README.md"],
    "input": "authenticated unchanged PR checked every 180 seconds for at least ten minutes; saved report of paired response statuses, REST x-ratelimit-used and GraphQL rateLimit.cost"
  },
  {
    "id": "full-gate",
    "title": "Run the complete Rust quality gate and TypeScript/documentation checks against the final branch diff",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main --title 'feat(epic): check GitHub on a fixed three-minute interval (#447)' && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'PATH=/root/.cargo/bin:/root/.bun/bin:$PATH /root/.bun/bin/bun run test'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8"],
    "depends_on": ["live-and-docs"],
    "paths": ["Cargo.lock", "crates/", "docs/", "plugins/epic-orchestrator/", "AGENTS.md"],
    "input": "the complete branch diff against origin/main with the live watch report and all loopback fixtures"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/{model,schedule,socket,server,snapshot,status,journal}.rs`, new GitHub watch and detector modules, and their colocated tests.
- `crates/epic-orchestrator/Cargo.toml`, `crates/core/github/src/{error,policy,cost}.rs`, and tests.
- `crates/cli/src/links.rs` if the tick reference must be carried into the resident engine.
- `plugins/epic-orchestrator/README.md`, generated `docs/cli/`, and a fixed-check design note under `docs/toolu/`.

## Verification

The isolated HTTPS service proves real ETag, retry, GraphQL and state transitions; virtual time proves the exact schedule and idle behavior. A live ten-minute watch produces `docs/toolu/evidence/2026-10-08-github-live-watch.json` with the UTC start/end times and one row per check (`restNotModified`, raw `restUsed`, adjacent `restUsedDelta`, `graphqlCost`). The step check validates duration, at least four checks, positive GraphQL cost per check, an authorized `304` in each later slot, at least two paired `304` slots, and a zero REST primary counter delta within every slot. The Rust gate enforces the repository's size, layout, coverage, CLI and quality rules. `bun run test` covers the TypeScript and documentation gates. Delivery makes a scoped commit, verifies the final ledger across the whole branch, runs the version-2 review and ready verdict, then pushes and opens the authorized PR before babysit.

## Deviations

- The `watch-clock` check now runs every `github_` test, including the registered-graph discovery and restart case. This strengthens the evidence for the same step without changing its scope.
- The live watch uses the unchanged, merged `Falconiere/toolu#454` PR. No authorized sandbox PR was available; #454 was already the read-only PR used for the spec's GraphQL cost observation. The report records its identity and response counters without changing that PR.
- The authenticated token's absolute REST `used` counter rose from 2 to 3 to 5 across `304` slots while another worker was active. [GitHub's REST guidance](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api#use-conditional-requests) says an authorized `304` costs no primary point; its [rate-limit guidance](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api#checking-the-status-of-your-rate-limit) says counters can vary by region. The live check therefore records raw counters and verifies a zero `used` delta across adjacent requests ending in `304` in each scheduled slot. Two post-warm-up slots returned paired `304`s. The final slot's first GET returned `200` although the PR's `updatedAt` remained October 5; its immediate conditional retry returned `304` with no increase in `used`.
- On this root host, Bun's filesystem permission tests require dropping DAC and ptrace capabilities so they observe normal user permissions; the plan's docs and full Bun checks run under `capsh` with the pinned Cargo/Bun paths. The earlier direct Bun run failed only on those root-only permission assumptions; the capability-limited full Bun gate passed.
