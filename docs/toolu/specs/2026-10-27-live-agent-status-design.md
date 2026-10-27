# Live agent status endpoint — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Claude (epic worker, Falconiere/toolu#651)   **Topic:** Each worker's agent status in `World` and a cached `GET /api/v1/agents/live` for terminal titles

## Problem

The dashboard (epic #649) must show what every worker's agent is doing right
now. The engine's herdr subscription already receives every status change. It
keeps only `issue.queue.agent_working: bool`, so the status word (`idle`,
`blocked`, `done`) is lost. The terminal title, which Claude Code and Codex set
to a live task summary, never reaches the engine at all.

## Non-Goals

1. No dashboard UI. #655 renders this data.
2. No screen reads (`agent.read`). That is #654, which reuses the limiter added here.
3. No key presses, and no herdr call from the state thread.
4. No peer proxy for `agents/live`. The route is local only.
5. No persistence of `agent_status` across restarts. The first `session.snapshot` after a (re)subscription re-seeds it.
6. No CLI fallback. The HTTP client speaks the socket at protocol 22 only.

## Architecture

**World status (subscription path).** `Issue` gains `agent_status: String`
(default `unknown`) and `agent_status_at_ms: u64`. A helper
`set_agent_status(issue, word, now_ms)` normalizes the word to
`idle|working|blocked|done|unknown`. It moves `agent_status_at_ms` only when
the word changes, and it keeps `queue.agent_working` equal to
`word == "working"`. `source_apply.rs` calls it:
- in `bind_or_gone` (snapshot): with the row's `agent_status` when the agent is listed, otherwise `unknown`;
- in `status_event` (`pane.agent_status_changed`);
- in `outage`: every issue becomes `unknown`.

The same `apply` journals the `herdr-event` row, so the issue document changes
within that one journal event. `status_api::issue_document` and the
`status_epics::detail` issue rows add `agent_status` and `agent_status_at`
(ISO UTC, or `null` before the first change).

**Live endpoint (HTTP path).** A new module `http_live.rs` holds:
- `Herdr` mode: `Off` (`TOOLU_EPIC_HERDR=off`, or `source::prompt_enabled()` is false), `Scripted` (`TOOLU_EPIC_SCRIPTS` is set), or `Socket(Env)`. `control::http_config` picks the mode. Tests inject `Socket(fixture env)`.
- `Limiter`: an `AtomicUsize` capped at 8, with `try_acquire() -> Option<Permit>` (RAII). It lives on `http::Context` so #654 can share it.
- `LiveCache`: `Mutex<Option<(Instant, Value)>>` plus a `fill: Mutex<()>`.

A request flows like this:
1. Off or scripted: answer `available:false` at once, with no herdr call and no slot.
2. Fresh cache (under 2 s old): serve it.
3. Otherwise take a limiter slot without blocking. If none is free, answer `503 herdr_busy`.
4. Lock `fill`, then re-check the cache. If another request filled it meanwhile, serve that.
5. Otherwise call herdr: `ping` on one connection (the protocol must equal 22), then `agent.list` on a second. Each connection has a 1 s read/write timeout and uses `herdr::socket_path(env)`, which honors `TOOLU_EPIC_HERDR_SESSION`.
6. Lock the `World` briefly to map rows to issues by `issue.agent == row.name`, build the document, and cache it (unavailable results included).

The decisive trade-off is the limiter-before-fill order. Requests that wait on
the fill lock still hold slots. That makes AC4's ninth overlapping request get
`503`, while the re-check under `fill` keeps herdr at one call per cache window
(design B, single-flight without slots, could never return 503 on its own).
Reused: `herdr::roundtrip`, `herdr::socket_path`, `HERDR_PROTOCOL`,
`secrets::redact_text`, `Response::error`, `json_response`, and the
`HerdrFixture` test server (with `terminal_title_stripped` added to its rows).

## Interfaces / Schema

```json
GET /api/v1/agents/live → 200
{"schema":1,"available":true,"sampled_at":"2026-10-27T14:00:00.000Z",
 "items":[{"key":"falconiere-toolu-651","epic":"falconiere-toolu-649","host":"claude",
           "agent_status":"working","title":"Live status endpoint","sampled_at":"2026-10-27T14:00:00.000Z"}]}
→ 200 {"schema":1,"available":false,"reason":"off|scripted|not running|protocol 23","sampled_at":"…","items":[]}
→ 503 {"error":"herdr_busy"}
```

- `host` is `issue.kind`, or `null` when empty.
- `title` is `terminal_title_stripped` with configured secrets redacted, then cut to 160 characters. It is `null` when absent or empty.
- `/api/v1/issues/{key}` and the `issues[]` rows of `/api/v1/epics/{id}` add `agent_status` and `agent_status_at`.
- `http::Config` gains `herdr: http_live::Herdr`.
- Authorization reuses `respond`: free on loopback, bearer token off loopback.
- `http_fleet::ROUTES` is unchanged.

## Failure modes and edge cases

- Socket missing or refused (herdr not installed, or server stopped): `available:false, reason:"not running"`, within the 1 s connect/timeout. This result is cached for 2 s too.
- Ping protocol ≠ 22: `reason:"protocol <n>"`. No CLI is spawned and nothing is journaled.
- Ping or `agent.list` times out, returns a non-JSON reply, or returns an error object: `reason:"not running"` for I/O failures, or `"error: <code>"` for a herdr error object.
- A herdr agent with no `name`, or a name that matches no issue: omitted.
- An unknown status word: `unknown`.
- The world mutex is poisoned: `500 status_unavailable` (existing behavior).
- A title of 0 characters, of exactly 160, or of over 160 multibyte characters: cut by `chars()`, never on a byte boundary.
- A secret split by the cut: redaction runs before the cut, so no partial secret survives.
- Nine overlapping stale requests: at most 8 hold slots and the rest get 503. A slow herdr is bounded by the 1 s timeouts, so slots free up quickly.
- `/api/v1/overview` never touches the limiter or `fill`, so a slow herdr cannot delay it.
- A herdr outage: every issue's `agent_status` becomes `unknown` and `agent_working` becomes false.

## Acceptance criteria

- **AC-1:** A protocol-22 herdr server with a worker agent mapped by name: a status change from `idle` to `working` and back is reported by `/api/v1/agents/live` (once the 2 s cache expires), and the issue document's `agent_status` changes with the `herdr-event` journal row of that change, without any `/agents/live` request.
- **AC-2:** Two `/api/v1/agents/live` requests within 2 s return the same `sampled_at`, and the herdr server logs exactly one `agent.list` call.
- **AC-3:** With the herdr socket absent, separately with mode `Off`, and with mode `Scripted`, the endpoint answers `200` with `available:false` and a `reason` within 2 s. Protocol 23 answers `reason:"protocol 23"` and spawns no CLI.
- **AC-4:** With all 8 limiter slots held, a stale `/api/v1/agents/live` answers `503 herdr_busy`, while `/api/v1/overview` answers `200` within 500 ms.
- **AC-5:** A title that contains the configured status token comes back with `[REDACTED]`, and a 200-character title comes back as 160 characters.
- **AC-6:** `docs/epic-status-api.md` records the probe results for Claude Code, Codex, Cursor Agent and Muse: the `agent_status` values seen through a turn, and each host's title behavior.
- **AC-7:** `cargo xtask gate` passes.
- **AC-8:** On `Fact::Outage`, every issue's `agent_status` becomes `unknown`.
- **AC-9:** On a non-loopback bind, `/api/v1/agents/live` without the bearer token answers `401`; and `/api/v1/fleet/peers/<peer>/agents/live` answers `404`, because the route is never proxied.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `HerdrFixture` (protocol-22 Unix socket, real agent process) plus the real `source_apply::apply` on an `Engine`; then a live probe: isolated real herdr 0.9.5 session (`herdr --session toolu-651-probe server`), real `toolu epic engine` with `TOOLU_EPIC_HERDR_SESSION`, a named agent mapped to a registered issue | statuses idle→working→idle reach both surfaces within 2 s | agent unlisted in the snapshot → `unknown` | `cargo test -p toolu-epic-orchestrator http_live` and `source_apply`; probe log `docs/toolu/evidence/2026-10-27-live-agent-status.md` |
| AC-2 | `HerdrFixture` + `http::start` | equal `sampled_at`, `calls()` contains one `agent.list` | request after expiry → new `sampled_at` | `http_live` tests |
| AC-3 | absent socket path, `Herdr::Off`, `Herdr::Scripted`, fixture with `protocol=23`; live engine with the probe session stopped and with `TOOLU_EPIC_HERDR=off` | `available:false`, reason, under 2 s | — | `http_live` tests; probe log |
| AC-4 | `Limiter` with 8 held permits + a running listener | 503 `herdr_busy`; overview 200 in under 500 ms | permit drop frees the slot | `http_live` tests |
| AC-5 | `Secrets` loaded from `TOOLU_EPIC_STATUS_TOKEN`, fixture title | redacted; 160 chars | multibyte title | `http_live` tests |
| AC-6 | live `herdr agent list` against real Claude Code, Codex, Muse and Cursor Agent sessions | table in docs | host that sets no title | `rg "Host probe" docs/epic-status-api.md` |
| AC-7 | full branch | gate green | — | `cargo xtask gate` |
| AC-8 | `Engine` with two issues, `apply(Fact::Outage)` | both `unknown` | — | `source_apply` tests |
| AC-9 | listener on `0.0.0.0:0` with a status token; listener with one configured peer | 401 without token, 200 with; 404 for the peer route | — | `http_live` tests |

## Documentation impact

`docs/epic-status-api.md` is generated from `status_schema.rs`. Add the
`/api/v1/agents/live` row, the new issue fields, and a "Live agent status"
section with the unavailable reasons, the limiter, the cache, and the host
probe table. Regenerate with `TOOLU_UPDATE_DOCS=1`.

## Open Questions

- Should `sampled_at` also appear per item? Yes: the issue lists it per item, so both levels carry it. Decided, non-blocking.
- Should herdr be reported "not installed" separately from "server down"? No: both are only visible as a missing socket in socket-only mode, so both report `not running`. Decided by the worker, non-blocking.

## Spec review (2026-10-27)

- Acceptance criteria: 🟡 should-fix: the scripted unavailable state had no evidence. Fixed: AC-3 now covers `Scripted`.
- Acceptance criteria: 🟡 should-fix: the issue's local-only and authorization rules had no AC. Fixed: AC-9 was added.
- Acceptance evidence: 🟡 should-fix: the issue's test plan asks for a real engine and a real herdr session, but AC-1 and AC-3 relied on the fixture. Fixed: a live probe log was added.
- Interfaces: 🔵 consider: the reason `error: <code>` for a herdr error object goes beyond the issue's list. Kept, because it is diagnostic only and still `available:false`.

**Status:** Approved
