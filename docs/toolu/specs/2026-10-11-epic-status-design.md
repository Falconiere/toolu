# Epic status server and dashboard — Design

**Date:** 2026-10-11   **Status:** Draft   **Author:** Codex   **Topic:** Issue #449, read-only epic status

## Problem

The resident engine has a local control socket and journal, but no view across registered epics, workers, hosts and machine resources. The current `status` document contains only phase and stage. A browser or another tool cannot watch transitions without reading private files or polling the watcher.

## Non-Goals

1. HTTP mutations, CORS access, notification delivery or remote fleet aggregation.
2. Implementing merge, rebase and queue actions assigned to #448. This issue reports queued ready work using the available state; #448 may replace that projection with its queue state.
3. Returning raw graph, issue, brief, environment, herdr pane or resource-store documents.

## Architecture

The existing state thread remains the only `World` writer. After every state step it publishes a snapshot and notifies stream readers. A small HTTP/1.1 listener reads that snapshot, registered epic files, journal records and the shared resource store outside the state lock. It owns no other network listener and starts with `epic engine` by default; `epic serve` is the explicit foreground entry. `--http <ADDR>` overrides the configured bind and port; `--no-http` disables it. The existing `epic.http.bind` and `epic.http.port` from #463 provide defaults, and #463's `Secrets` resolves the bearer token. The local control socket uses the same read-only projection functions for `epic status`.

Each accepted connection gets a bounded blocking handler with read/write deadlines. The listener caps simultaneous connections at 64 and streams at 8. Excess clients receive `503` and close. Keep-alive supports 50 polling clients without creating 50 long-lived idle handlers. A stream reads retained journal records after its cursor, then waits no more than one second for new records; it sends comment heartbeats at least every 15 seconds. Journal sequence stays monotonic across day boundaries and process restarts.

Every response is assembled from allow-listed fields. Worker notes are cut to 160 characters and redacted with #463's resolved secrets. Journal tokens, graph excerpts, worker brief contents, environment, notification URLs and pane text never enter the projection. Any `401`, `403`, `404`, `405`, `413`, `429`, `503` or `500` body is a fixed error code and contains no user data.

## Interfaces / Schema

All JSON responses contain integer `schema: 1`; times are UTC ISO-8601 strings or `null`, ages are non-negative integer milliseconds, and absent optional facts are `null` or empty arrays. Every HTTP response includes `Cache-Control: no-store`; none includes a CORS header. `GET` and `HEAD` use the same status and headers, but `HEAD` has no body. Unsupported methods return `405` with `Allow: GET, HEAD`. Unknown routes return `404`.

| Path | Document fields after `schema` |
|---|---|
| `/healthz` | `version`, `uptime_ms`, `journal_seq` |
| `/api/v1/overview` | `engine`, `journal_seq`, `paused`, `epics` (key, stage counts, attention count), `counts` by stage, `attention_count`, `hosts`, `machine` |
| `/api/v1/epics` | `items` (epic key, paused, stage counts, attention count) |
| `/api/v1/epics/{id}` | `key`, `paused`, `graph` (safe issue keys, titles, blockers and waves), `issues` (safe summaries), `queue` |
| `/api/v1/issues/{key}` | `key`, `epic`, `stage`, `phase`, `phase_age_ms`, `host`, `model`, `effort`, `pr`, `last_gate_verdict`, `queue_position`, `launches`, `phase_history`, `recent_events` |
| `/api/v1/agents` | `items` (state, key, epic, worktree, session_id) |
| `/api/v1/hosts` | `items` for Claude Code, Codex, Cursor Agent and OpenCode: `host`, `cap`, `running`, `cooldown_until`, `reason` |
| `/api/v1/machine` | `load`, `available_memory_bytes`, `steal`, `pressure` (held and reason), `policy` (agent/job caps), `agent_leases`, `job_leases` (owner, worktree, stage and age; no token) |
| `/api/v1/jobs` | `running`, `waiting` (key, epic, worktree, wait age) |
| `/api/v1/queue` | `repositories` (repo and ordered issue keys with positions) |
| `/api/v1/attention` | `items` (kind, key, epic, bounded note, seq, age) |
| `/api/v1/events?since=S&limit=N` | `events`, `oldest_seq`, `next_seq`; default `since=0`, `limit=100`, maximum 500 |

An event has only `seq`, `at`, `kind`, `name`, `epic`, `key`, and bounded `note`. `/api/v1/events/stream` emits each as `id: <seq>`, `event: journal`, `data: <event JSON>`, in ascending sequence. `Last-Event-ID` takes precedence over `since`. The server returns the retained events after that sequence; if the requested sequence predates retention, it starts at `oldest_seq` and emits a `reset` event identifying that boundary. Malformed cursors or limits return `400`.

`toolu epic status --json` without an argument returns the overview over the control socket; an epic key returns `/api/v1/epics/{id}`, and an issue key returns `/api/v1/issues/{key}`. The global `--json` flag remains accepted. The command preserves the existing `engine`, `paused`, `epics`, `issues` and `attention` fields in the no-argument document for existing consumers. Unknown keys return a clear failure.

The listener accepts a `Host` matching the actual local socket address and port; loopback also permits `localhost` and `127.0.0.1`. A foreign `Host` returns `403`. A non-loopback bind, including `0.0.0.0`, fails startup unless #463 supplies a status bearer token. On that listener the `Authorization: Bearer <token>` value is checked in constant time, with missing or invalid values returning `401`. Loopback needs no token. Headers are capped at 8 KiB, request targets at 2 KiB, and bodies are rejected.

`docs/epic-status-api.md` is generated from the Rust response type/schema definitions and checked for drift in the gate. The single HTML page at `/` has embedded CSS and JavaScript, no external requests except same-origin API calls, and displays the overview, epic boards, hosts, machine, queue and attention. It opens one `EventSource`, fetches changed documents on events, and reconnects from the last event ID without reloading.

## Failure modes and edge cases

- No registered epics produces empty arrays and zero counts; `healthz` still works.
- Missing optional route, agent, pressure, queue or gate data produces `null` or empty fields. Invalid required resource state or registered graph returns a fixed `500` response and a diagnostic on stderr, never partial raw data.
- Malformed HTTP, duplicate `Host` or `Authorization`, unsupported transfer encoding, an overlong request, and an over-cap connection are rejected before reading private data.
- Listener bind failure stops `epic engine` with the bind address and an error, without printing credentials. `--no-http` still allows the control socket to run.
- A stream disconnect releases its slot; a slow reader cannot retain unlimited events or delay the state thread. `Last-Event-ID` replay preserves order across journal rotation and engine restart within the 90-day retention window.
- Secrets rotated while the engine is running take effect on restart; public fields remain redacted with the startup-resolved set until then.

## Acceptance criteria

- **AC-1:** Given a scripted epic and resource state, every listed JSON endpoint returns its documented versioned schema, and `epic status` returns the matching overview, epic or issue document through the control socket.
- **AC-2:** Given a worker phase report, an open dashboard updates the phase within two seconds through the event stream without a page reload.
- **AC-3:** Given journal events across a date boundary and an SSE reconnect with `Last-Event-ID`, every retained later event arrives once, in increasing sequence order.
- **AC-4:** Given a non-loopback bind without a token, engine startup fails clearly; with a token, missing or invalid authentication gets `401`, and valid authentication reads status.
- **AC-5:** Given a foreign `Host` and unsupported methods, responses are `403` and `405` respectively, with no CORS header or cacheable response.
- **AC-6:** Given canary values in the environment and a worker brief body, no endpoint, event or HTML response contains either value; worker notes are bounded and configured secrets are redacted.
- **AC-7:** Given 50 concurrent keep-alive polling clients and eight streams, the Linux server stays within 13 MiB max RSS and 7 ms p90 polling latency, using #410's measurement method.
- **AC-8:** Given a host cooldown and a waiting expensive job in the resource store, `/api/v1/hosts` and `/api/v1/jobs` expose their end/reason and wait age.
- **AC-9:** Given the new CLI entries and skill text, generated CLI and API docs match the binary and the Markdown–CLI drift gate passes.

## Acceptance evidence

| AC | Real input and expected observation | Runnable check |
|---|---|---|
| 1 | Temporary registered epic with `graph.json`, issue/status/route files and policy/state leases; snapshots pin every response and the control socket output. | `cargo test -p toolu-epic-orchestrator status_api` |
| 2 | Launch the real `toolu epic engine`, submit a real `report` via its Unix socket, and run the embedded page script in a browser or DOM-capable harness; phase visible before a two-second deadline. | `cargo test -p toolu-epic-orchestrator dashboard_live` |
| 3 | Real daily JSONL files and an HTTP reconnect with `Last-Event-ID`; compare received IDs exactly. | `cargo test -p toolu-epic-orchestrator stream_replay` |
| 4 | Real TCP listener and token from a mode-0600 config file; observe start error, 401 and 200. | `cargo test -p toolu-epic-orchestrator http_auth` |
| 5 | Raw TCP requests with hostile Host and methods; inspect status and headers. | `cargo test -p toolu-epic-orchestrator http_boundary` |
| 6 | Plant distinct canaries in environment and brief, request every route and stream event, inspect response bytes. | `cargo test -p toolu-epic-orchestrator public_canaries` |
| 7 | Release binary on Linux, 50 keep-alive clients × 40 requests plus eight streams; read `VmHWM` and calculate client p90. | `cargo test -p toolu-epic-orchestrator --test status_budget` in a Linux job lease. |
| 8 | Real policy and resource state fixture with cooldown and job waiter, inspected over HTTP. | `cargo test -p toolu-epic-orchestrator resource_status` |
| 9 | Regenerate docs from binary and Rust schema, then run drift gates. | `cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo xtask gate` |

## Documentation impact

Add generated `docs/epic-status-api.md`, update `docs/epic-orchestrator/README.md` and the epic skill and command text to use `toolu epic status --json` in place of watcher peek. Regenerate `docs/cli/` from the binary.

## Open Questions

None blocking. The exact queue state from #448 is not yet available in this branch, so the read-only queue projection uses ready issue records and documents that source.
