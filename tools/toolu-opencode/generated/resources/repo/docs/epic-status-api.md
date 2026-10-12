# Epic status API

Generated from `crates/epic-orchestrator/src/status_schema.rs`. Regenerate with `TOOLU_UPDATE_DOCS=1 cargo test -p toolu-epic-orchestrator status_api_doc`.

The resident engine serves HTTP/1.1 on `127.0.0.1:7717` by default. `toolu epic serve --http ADDR` selects another address; `--no-http` disables the listener. Non-loopback binds require a status token from `toolu/secrets.json` or `TOOLU_EPIC_STATUS_TOKEN` / `TOOLU_EPIC_TOKEN`, sent as `Authorization: Bearer <token>`.

Only GET and HEAD are accepted. HEAD has no body. Every JSON body contains `schema: 1`; every response has `Cache-Control: no-store` and no CORS header. Errors use fixed JSON codes. The listener accepts only its bound Host and loopback aliases.

| Path | Purpose | Fields after `schema` |
|---|---|---|
| `/healthz` | Engine health | version, uptime_ms, journal_seq |
| `/api/v1/overview` | Current aggregate | engine, paused, journal_seq, epics, issues, attention, counts, attention_count, hosts, machine, jobs |
| `/api/v1/epics` | Registered epics | items (key, paused, counts, attention_count) |
| `/api/v1/epics/{id}` | One epic board | key, paused, graph, issues, queue |
| `/api/v1/issues/{key}` | One worker | key, epic, stage, phase, phase_age_ms, host, model, effort, pr, last_gate_verdict, queue_position, launches, phase_history, recent_events |
| `/api/v1/agents` | Worker sessions | items (state, key, epic, worktree, session_id) |
| `/api/v1/hosts` | Host capacity | items (host, cap, running, cooldown_until, reason) |
| `/api/v1/machine` | Shared resources | load, available_memory_bytes, steal, pressure, policy, agent_leases, job_leases |
| `/api/v1/jobs` | Expensive jobs | running, waiting |
| `/api/v1/queue` | Ready work by repository | repositories (repo, items) |
| `/api/v1/attention` | Live attention | items (kind, key, epic, note, seq, state, age_ms) |
| `/api/v1/events` | Journal replay | events, oldest_seq, next_seq |
| `/api/v1/events/stream` | Live journal feed | SSE id, event, data |

`/api/v1/events` accepts `since` (default 0) and `limit` (default 100, maximum 500). An event contains `seq`, `at`, `kind`, `name`, `epic`, `key`, and a bounded `note`. The stream emits `id: <seq>`, `event: journal`, and `data: <JSON>`; `Last-Event-ID` overrides `since`. A cursor older than retained history receives a `reset` event.

The dashboard at `/` uses these same-origin endpoints. Waiting jobs are recent refused admissions; their markers expire after two minutes. Queue entries are the engine's merge queue: per repository, ready issues with a pull request, ordered by `unblocks`, then ready time, then key; `head` marks the one the engine works.
