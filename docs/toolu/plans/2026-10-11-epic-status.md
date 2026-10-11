# Epic status server and dashboard

**Date:** 2026-10-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-11-epic-status-design.md   **Topic:** Issue #449 read-only HTTP and control-socket status

## Evidence and approach

The approved spec is the contract. `socket.rs` already publishes a cloned `World`; `journal.rs` owns ordered events; `snapshot.rs` reads registered epics; `toolu_engine::resources::store` owns policy and leases; #463's `epic` config and `secrets` own listener settings and token resolution. The real #402 epic files show which fields need a public allow-list. The existing job runner records admitted leases only, so waiting status needs a bounded sidecar written by both managed-job entry points. The Linux status-server budget is 7 ms p90 and 13 MiB high-water RSS under 50 polling clients.

Jev was unavailable: the installed `toolu jev` has only `planned`. No delegation is needed; the work has one state owner and an order-dependent HTTP integration path.

## Workstream summary

First make safe status projections and replayable journal reads. Then wire the bounded listener and stream to the resident engine, add the page and control-socket selector, record waiting jobs, generate documentation, and measure the real binary.

## Steps (machine-readable)

```json
[
  {
    "id": "journal",
    "title": "Keep journal sequence monotonic and page retained events without private tokens",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_journal",
    "ac_refs": ["AC-3", "AC-6"],
    "paths": ["crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/tests/journal_test.rs", "crates/epic-orchestrator/src/status_events.rs", "crates/epic-orchestrator/src/tests/status_events_test.rs"],
    "input": "Real dated JSONL files with a partial line, a day gap, tokens and a reconnect cursor"
  },
  {
    "id": "documents",
    "title": "Project safe overview, epic, issue, agent, host, machine, queue and attention documents",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_api",
    "ac_refs": ["AC-1", "AC-6", "AC-8"],
    "depends_on": ["journal"],
    "paths": ["crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/status_api.rs", "crates/epic-orchestrator/src/status_epics.rs", "crates/epic-orchestrator/src/status_resources.rs", "crates/epic-orchestrator/src/tests/status_api_test.rs", "crates/epic-orchestrator/src/tests/status_epics_test.rs", "crates/epic-orchestrator/src/tests/status_resources_test.rs"],
    "input": "Registered temporary epic with real graph, issue, status and route JSON plus resource policy, leases, pressure and cooldown"
  },
  {
    "id": "listener",
    "title": "Start a bounded HTTP listener with Host, method, size and bearer-token boundaries",
    "check": "cargo test -p toolu-epic-orchestrator --lib http_",
    "ac_refs": ["AC-4", "AC-5", "AC-6"],
    "depends_on": ["documents"],
    "paths": ["crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/http.rs", "crates/epic-orchestrator/src/http_access.rs", "crates/epic-orchestrator/src/http_request.rs", "crates/epic-orchestrator/src/http_response.rs", "crates/epic-orchestrator/src/socket.rs", "crates/epic-orchestrator/src/socket_herdr.rs", "crates/epic-orchestrator/src/control.rs", "crates/epic-orchestrator/src/tests/http_test.rs", "crates/epic-orchestrator/src/tests/http_access_test.rs", "crates/epic-orchestrator/src/tests/http_request_test.rs"],
    "input": "Real TCP requests to a loopback and wildcard-bound engine, including foreign Host, missing token, unsupported method and oversize headers"
  },
  {
    "id": "stream",
    "title": "Replay SSE from Last-Event-ID, heartbeat and update the embedded dashboard",
    "check": "cargo test -p toolu-epic-orchestrator --lib stream_replay && cargo test -p toolu-epic-orchestrator --lib dashboard_live",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["listener"],
    "paths": ["crates/epic-orchestrator/src/http.rs", "crates/epic-orchestrator/src/http_stream.rs", "crates/epic-orchestrator/src/dashboard.rs", "crates/epic-orchestrator/src/socket.rs", "crates/epic-orchestrator/src/tests/http_stream_test.rs", "crates/epic-orchestrator/src/tests/http_test.rs"],
    "input": "A real engine, two HTTP stream clients and a worker report over the Unix control socket"
  },
  {
    "id": "cli",
    "title": "Expose epic serve and shared status documents through the control socket",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_cli",
    "ac_refs": ["AC-1", "AC-4", "AC-9"],
    "depends_on": ["documents", "listener"],
    "paths": ["crates/epic-orchestrator/src/verbs.rs", "crates/epic-orchestrator/src/query.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/control.rs", "crates/epic-orchestrator/src/tests/query_test.rs", "crates/epic-orchestrator/src/tests/verbs_test.rs"],
    "input": "A real local engine and status queries for all epics, one epic and one issue"
  },
  {
    "id": "waiting",
    "title": "Record and expose waiting expensive jobs from both Rust and Bun managed-job entry points",
    "check": "cargo test -p toolu-epic-orchestrator --lib resource_status && bun test plugins/epic-orchestrator/scripts/__tests__/job-waiting.test.ts",
    "ac_refs": ["AC-8"],
    "depends_on": ["documents"],
    "paths": ["crates/epic-orchestrator/src/job.rs", "crates/epic-orchestrator/src/waiting_jobs.rs", "crates/epic-orchestrator/src/status_resources.rs", "crates/epic-orchestrator/src/tests/waiting_jobs_test.rs", "plugins/epic-orchestrator/scripts/job.ts", "plugins/epic-orchestrator/scripts/__tests__/job-waiting.test.ts"],
    "input": "Two real job processes bound to one resource root with one expensive-job slot"
  },
  {
    "id": "docs",
    "title": "Generate API and CLI references and update epic Markdown to use status",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_api_doc_matches_schema_definitions && cargo xtask gate --only docs-cli && cargo xtask check-markdown-cli",
    "ac_refs": ["AC-9"],
    "depends_on": ["cli", "stream", "waiting"],
    "paths": ["docs/epic-status-api.md", "docs/cli/epic.md", "docs/cli/commands.json", "docs/epic-orchestrator/README.md", "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/epic-orchestrator/commands/epic.md", "crates/epic-orchestrator/src/status_schema.rs", "crates/epic-orchestrator/src/tests/status_schema_test.rs"],
    "input": "Rust status projection types and the real toolu epic command tree"
  },
  {
    "id": "budget",
    "title": "Measure the release listener under 50 keep-alive clients and eight streams",
    "check": "cargo build --release -p toolu-cli && TOOLU_RELEASE_BIN=target/release/toolu cargo test -p toolu-epic-orchestrator --test status_budget",
    "ac_refs": ["AC-2", "AC-7"],
    "depends_on": ["stream", "docs"],
    "paths": ["crates/epic-orchestrator/tests/status_budget.rs", "crates/epic-orchestrator/src/http.rs", "crates/epic-orchestrator/src/http_stream.rs", "docs/resource-budgets.md"],
    "input": "Release toolu epic engine with 50 real keep-alive polling clients, eight SSE sockets and Linux /proc VmHWM"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/journal.rs`, `status.rs`, `status_api.rs`, `status_resources.rs`, `http.rs`, `http_request.rs`, `http_response.rs`, `http_stream.rs`, `verbs.rs`, `query.rs`, `dispatch.rs`, `socket.rs`, `control.rs`, `job.rs` and `lib.rs`
- `crates/epic-orchestrator/src/dashboard.rs` and colocated `src/tests/*_test.rs`
- `crates/epic-orchestrator/tests/status_budget.rs`
- `plugins/epic-orchestrator/scripts/job.ts` and its real-process test
- `docs/epic-status-api.md`, generated `docs/cli/`, epic README, skill and command

## Verification

Each behavior step starts with a failing real-input test and ends green. Run the CLI and HTTP tests against a temporary resource root and a spawned engine; the dashboard's real JavaScript behavior must also be checked with `agent-browser` against that engine. On Linux, measure client latency and `VmHWM` through the shared job lease. Regenerate the API and CLI docs, run `cargo xtask check-markdown-cli`, then run the full `cargo xtask gate` under the shared lease. The final ledger verification and review happen after a scoped commit and before push; verify the default-branch PR and hand it to babysit only after the verdict is ready.

## Deviations

- The page is embedded in `dashboard.rs` because this crate forbids source includes. It remains a single asset-free page served at `/`.
- Waiting admission records use one atomic file per issue under `waiting-jobs/`. A shared JSON file would require another writer lock across the Rust and Bun entry points. The reader allow-lists fresh records and removes stale entries from its response.
- The generated API reference is rendered by the Rust schema test; setting `TOOLU_UPDATE_DOCS=1` updates the file, and the ordinary test checks exact equality. The browser's actual JavaScript was verified with `agent-browser` against a real engine because a Rust unit test cannot execute it.
- The Linux budget runs inside a disposable Rust 1.99 container from a copy of this worktree. The approved ledger check still compiles the test on macOS; its assertion runs on Linux.
