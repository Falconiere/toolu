# Epic fleet view — Design

**Date:** 2026-10-13   **Status:** Approved   **Author:** Claude Code   **Topic:** Issue #453, one status page for engines on several machines

## Problem

Epics run on more than one machine, and each resident engine serves only its own status (#449). The user opens one dashboard per machine and cannot see in one place which machine runs which issue, which hosts are busy, or where attention is waiting.

## Non-Goals

1. Acting on a peer. Every peer request is a `GET`; no control-socket op, mutation or acknowledgement crosses machines.
2. Proxying a peer's `/api/v1/events/stream`, `/api/v1/events` or `/api/v1/metrics`. The peer view polls.
3. Peer discovery, TLS termination in the engine, a configurable accepted `Host`, or a persisted last-seen time. Peers come only from `epic.peers`; last-seen lives in the listener's memory since engine start.
4. Changing the `epic.peers` or `peer_tokens` formats of #463.
5. Embedding whole peer overviews in `/api/v1/fleet`. The issue's "each peer's overview" is served as an allow-listed projection; a peer's full documents are reachable only through the proxy routes.

## Architecture

New code lives in new modules of `crates/epic-orchestrator`; existing files gain only hooks. Line budget (code lines, limit 300): `http.rs` 290 → about 282 after `json_response` moves to `http_response.rs` (123 → about 140) and the fleet hook is added; `verbs.rs` 298 → about 289 after `status_command` moves to `query.rs` (264 → about 280 with the `--fleet` branch). `http_response::reason` gains `502 Bad Gateway`.

- **`fleet.rs`** — peer fetching and the fleet document. `fetch(client, peer, token, route)` sends one `GET {peer.url}api/v1/{route}` through `toolu_http::Client::send` with `Auth::Bearer(token)` when `Secrets::peer_token(name)` is set and `Auth::None` otherwise, and classifies the reply. `fetch_all` runs one `std::thread::scope` thread per peer, so a round lasts as long as the slowest peer, at most the client's 2 s whole-request deadline. `document(machines, now_ms, secrets)` builds the fleet JSON from an allow-listed projection of each overview, then applies `secrets::redact_json` with the local `Secrets`, which already include every peer token.
- **`http_fleet.rs`** — the listener side. `FleetCache` holds the last peer round (results and its time) and each peer's last `ok` time. `refresh(max_age)` takes the cache mutex, and if the last round completed more than `max_age` ago runs a new one while holding it, so concurrent callers share one round: a round's time is its completion instant, so a caller that waited for a slow round finds it fresh. A **refresher thread** (`http_fleet::Refresher`, whose `spawn` and `stop` keep `http.rs` to one field and two calls) starts only when peers are configured. It runs its first round immediately at listener start, then calls `refresh(25 s)` every 30 s (`thread::park_timeout`; `stop` raises the flag, unparks and joins), so `last_seen` is known even when no page is open. A `/api/v1/fleet` request calls `refresh(1 s)`. The local machine is computed on every request outside the cache: the world lock is taken, `status_api::document` is built, and the lock is released before the cache mutex is taken. `response(..)` answers `/api/v1/fleet` and `/api/v1/fleet/peers/{name}/{route}` and returns `None` for any other path, so `finite_response` gains one `if let`.
- **Proxy limit.** Proxied calls are not cached. At most 8 run at once per listener (an `AtomicUsize`, like the stream cap); a ninth gets `503 proxy_limit` at once. The 64-connection cap therefore keeps at least 56 slots for local routes even when every proxied call waits on a blackholed peer.
- **`fleet_cli.rs`** — `toolu epic status --fleet`: the CLI process gets the local overview through the control socket, as `status` does (an empty registry gives the engine-down `status::document`), reads `epic.peers` and `secrets::load` itself, then runs one `fetch_all` round and `document`. A network wait never blocks the engine's state thread, and the command works with `--no-http`. Peers that never answered in this run have `last_seen: null`.
- **`dashboard.rs`** — a machine switcher and fleet summary in the existing page.

The engine reads `epic.peers` once at startup, as it does for `epic.http`; `--fleet` reads it on every run, so after a config edit they differ until the engine restarts. The HTTP client is built with an empty `Env`, so peers are dialed directly and no proxy variable sees the request. Body cap 1 MiB; redirects 0.

Peer state from one reply:

| Reply | `state` | `reason` |
|---|---|---|
| `200`, a JSON object whose `schema` is the integer `1` | `ok` | `null` |
| `200` with non-JSON, a non-object, or `schema` absent or not `1`; `404` on `overview` | `version-mismatch` | `null` |
| `401` | `unauthorized` | `token` |
| `403` whose body is the engine's fixed `{"error":"foreign_host"}` | `unauthorized` | `host` |
| any other `403` (for example a proxy or firewall in front) | `unauthorized` | `null` |
| Connect error, timeout, body over 1 MiB, any `3xx`, any other status | `unreachable` | `null` |

## Interfaces / Schema

`GET /api/v1/fleet` — one peer `ok` with one in-flight issue, one never-answering peer:

```json
{
  "schema": 1,
  "summary": {
    "machines": 3, "reachable": 2,
    "hosts": [{"host": "claude", "running": 2, "cap": 6}],
    "in_flight": 2, "attention_count": 1
  },
  "machines": [
    {"name": "local", "local": true, "state": "ok", "reason": null, "age_ms": 0,
     "last_seen": "2026-10-13T12:00:00.000Z", "engine": "running", "paused": false,
     "journal_seq": 42,
     "epics": [{"key": "repo-402", "paused": false, "counts": {"running": 1}, "attention_count": 1}],
     "in_flight": [{"key": "repo-449", "epic": "repo-402", "stage": "running", "phase": "execution"}],
     "hosts": [{"host": "claude", "running": 1, "cap": 3}],
     "counts": {"running": 1}, "attention_count": 1},
    {"name": "server", "local": false, "state": "ok", "reason": null, "age_ms": 400,
     "last_seen": "2026-10-13T11:59:59.600Z", "engine": "running", "paused": false,
     "journal_seq": 9,
     "epics": [{"key": "repo-500", "paused": false, "counts": {"awaiting_merge": 1}, "attention_count": 0}],
     "in_flight": [{"key": "repo-501", "epic": "repo-500", "stage": "awaiting_merge", "phase": "ready"}],
     "hosts": [{"host": "claude", "running": 1, "cap": 3}],
     "counts": {"awaiting_merge": 1}, "attention_count": 0},
    {"name": "nas", "local": false, "state": "unreachable", "reason": null, "age_ms": null,
     "last_seen": null, "engine": null, "paused": null, "journal_seq": null,
     "epics": [], "in_flight": [], "hosts": [], "counts": {}, "attention_count": 0}
  ]
}
```

- The local machine is always first, named `local`, with `local: true`; peers follow in config order. A peer named `local` is told apart by `local: false`. Every machine calls itself `local`, so two machines' fleet documents read side by side both start with `local`; that is a known limitation.
- `state` is `ok`, `unreachable`, `unauthorized` or `version-mismatch`; `reason` is `token`, `host` or `null` as in the table. Data fields are filled only for `ok`; otherwise `engine`, `paused`, `journal_seq` are `null`, lists `[]`, `counts` `{}`, `attention_count` `0`.
- `last_seen` is the UTC ISO-8601 time of the peer's last `ok` reply to this listener since engine start (or to this CLI run), `null` if none. `age_ms` is `now − last_seen` at document time, `null` if none; for an `ok` peer it is the age of the cached round (under 1 s for a request, up to 30 s otherwise). The local machine has `age_ms: 0`, `last_seen` now.
- Projection allow-list per `ok` machine: `engine` (string), `paused` (bool, else `false`), `journal_seq` (integer, else `0`); `epics[]` as `key`, `paused`, `counts`, `attention_count`; `in_flight[]` from `issues[]` whose `stage` is `running` or `awaiting_merge`, as `key`, `epic`, `stage`, `phase`; `hosts[]` from `hosts.items[]` as `host`, `running`, `cap`; `counts` (string → integer entries only); `attention_count` (integer, else `0`). A wrong-typed string field becomes `""`, a wrong-typed number `0`, a non-array list `[]`. Strings are cut to 160 characters. Every other peer field is dropped.
- Engine-down local machine (`--fleet` with an empty registry): `state: "ok"`, `engine: "down"`, `journal_seq: 0`, empty lists, `counts: {}`, `attention_count: 0`.
- `summary.machines` counts every machine, `reachable` the `ok` ones; `hosts` sums `running` and `cap` per host name over `ok` machines, sorted by name; `in_flight` and `attention_count` sum over `ok` machines.

`GET /api/v1/fleet/peers/{name}/{route}`:

- The path after `peers/` is split into `{name}` and `{route}` at the first raw `/`, before any decoding, so an encoded `%2F` stays inside the name. `{name}` is then percent-decoded (`%XX` to bytes, then UTF-8; `+` stays `+`) and must equal a configured peer name exactly; undecodable or unknown names are `404 not_found`. The dashboard encodes it with `encodeURIComponent`. `local` addresses a peer named `local`, never this machine.
- `{route}` is `overview`, `epics`, `agents`, `hosts`, `machine`, `jobs`, `queue`, `attention`, `epics/{id}` or `issues/{key}`, where `{id}`/`{key}` match `[A-Za-z0-9._-]{1,128}` and are not `.` or `..`. Anything else is `404 not_found`. A query string is ignored. The peer URL is built from these checked parts only.
- Reply: the peer's JSON document redacted with the local secrets, `200`. Errors are fixed bodies: a peer `404` on a detail route is `404 not_found`; `502 peer_unreachable`, `502 peer_unauthorized`, `502 peer_version_mismatch`; `503 proxy_limit`. `HEAD` makes the same peer call and sends the same status and headers without a body, as #449 does for every route.

`toolu epic status --fleet [--json]` prints the same document as `/api/v1/fleet` on stdout as one JSON line; the global `--json` is accepted as for `status`. `--fleet` conflicts with the `EPIC` argument (clap usage error, exit 64). It exits 0 whatever the peer states. A malformed `epic` config or secrets file fails with the parse error, exit 1 (as `status` and `engine` fail today).

Dashboard: when `/api/v1/fleet` lists more than one machine, the page shows a fleet summary (Machines, Hosts busy as running/cap, In flight, Needs attention) and a machine switcher with one tab per machine and its state. The local machine keeps the event stream, and its panels load from the local routes without waiting for `/api/v1/fleet`, which the page fetches separately. Selecting an `ok` peer fetches its panels through `/api/v1/fleet/peers/{name}/…` and re-polls every 5 s; selecting a peer that is not `ok` shows `Unreachable · last seen <time>` (`Unreachable · never seen` when `last_seen` is `null`; or `Unauthorized: token rejected` / `Unauthorized: host rejected` / `Unauthorized` / `Version mismatch`) in place of the panels and makes no proxy call. A peer's panel documents are fetched one after another, so one tab holds at most one proxy slot; a `502` or `503` on a panel keeps the last panels and shows a notice. The page fetches `/api/v1/fleet` on load, after each local journal event (the existing 100 ms debounce), and every 5 s.

## Failure modes and edge cases

- **No peers configured:** no refresher thread; `/api/v1/fleet` has only `local`; the dashboard hides switcher and fleet summary; `--fleet` prints the one-machine document.
- **Peer without a token:** sent with no `Authorization`; a peer requiring one answers `401` → `unauthorized`/`token`.
- **Blackholed peer:** the 2 s deadline ends its fetch; `/api/v1/fleet` returns within that bound (plus local work); local routes take no lock the fleet fetch holds, and the proxy limit keeps connection slots free.
- **Wrong Host:** a peer accepts only its bound address (`ip:port`, plus `localhost`/`127.0.0.1` on loopback), so a URL with a DNS name, or without the port, gets `403` → `unauthorized`/`host`. A TLS proxy in front of a peer must forward with `Host` set to the engine's address (nginx's default `proxy_pass` behaviour; Caddy `header_up Host {upstream_hostport}`).
- **Older or foreign API:** `schema` other than 1, non-JSON or overview `404` → `version-mismatch`, no data.
- **Hostile peer data:** typing and truncation as above; the result redacted with local secrets, so a peer echoing a local token shows `<redacted>`.
- **Redirect or oversized body:** `unreachable`.
- **Concurrent fleet requests:** one peer round at a time per listener; callers wait for it (≤ 2 s) and share it.
- **Concurrent fleet requests share a round:** N simultaneous `/api/v1/fleet` requests cause one `GET /api/v1/overview` per peer.
- **Poisoned cache mutex:** `500 status_unavailable`, as for the world lock; the refresher exits.
- **Plain HTTP:** a bearer token sent to an `http://` peer crosses the network in clear text; the docs say to use only private networks or TLS in front.
- **Tokens:** a peer token is used only inside `Auth::Bearer`, whose `Debug` is redacted; no fleet path writes stderr or the journal, and no error body carries peer bytes.

## Acceptance criteria

- **AC-1:** Given two in-process engines, each with a registered epic and an issue made `running` by a real `report`, and engine A configured with engine B as peer `server`, `/api/v1/fleet` on A lists `local` and `server`, both `ok`, each with its own in-flight issue key, and `summary.machines: 2`, `in_flight: 2`.
- **AC-2:** Given a blackholed peer and engine B reached with a wrong token, `/api/v1/fleet` returns within 3 s (2 s deadline + 1 s margin for shared CI) with them `unreachable` and `unauthorized`/`token` and the local machine's data present, and `/api/v1/overview` answers within 500 ms while that fleet request is in flight.
- **AC-3:** Given peers answering `/api/v1/overview` with `schema: 2`, with a `text/plain` 200, and with `404`, each is `version-mismatch` with no data; a `403 foreign_host` peer is `unauthorized`/`host` and another `403` is `unauthorized` with `reason: null`; a `302` peer and one sending more than 1 MiB are `unreachable`.
- **AC-4:** Given a distinct canary token per peer, and peers whose replies embed that canary in allow-listed fields (an issue key, a host name) and in a proxied document, no `/api/v1/fleet` body, proxied body, `--fleet` stdout or stderr, or file under engine A's resource root (journal included) contains a canary, while each peer's recorded request head contains `Authorization: Bearer <its canary>`; a peer with no token receives no `Authorization` header.
- **AC-5:** Given peer B `ok` once and then stopped, the next fleet document after the 1 s cache shows B `unreachable` with the earlier `last_seen` and `age_ms` at least the time elapsed since. Given a listener started with a fixture peer and no client traffic, the fixture records one `GET /api/v1/overview` within 1 s (the refresher's immediate first round). Given 5 simultaneous `/api/v1/fleet` requests on a fresh round, the fixture records exactly one more overview request; with a fixture peer that answers after 1.5 s, 5 simultaneous requests each return within 2.5 s and the fixture records one overview request.
- **AC-6:** Given engine B as peer `server`, `/api/v1/fleet/peers/server/epics/{id}` and `/issues/{key}` return B's documents; a percent-encoded peer name resolves; an unknown peer, a disallowed route, a `..` segment and a 129-character key return `404` while a 128-character key is proxied; a blackholed peer returns `502 peer_unreachable`; with 8 proxied calls waiting on a blackholed peer, a ninth gets `503 proxy_limit` while `/api/v1/overview` still answers `200`.
- **AC-7:** Given a `HOME` whose `toolu.config.json` names engine B, a closed port and a peer named `local` as peers, `toolu epic status --fleet` exits 0 with the fleet document (local engine-down entry with `local: true`, B `ok`, the closed port `unreachable`, the peer `local` with `local: false`); with an `EPIC` argument it is a usage error; with a credential key under `epic` it fails with exit 1.
- **AC-8:** Given two real `toolu epic serve` processes (engine A with engine B and a closed port as peers), the shipped page run by headless Chrome against engine A renders the machine switcher with `local`, `server` and the closed peer, the fleet summary (`2` reachable machines), and `Unreachable · never seen`; against engine B (no peers) the switcher is hidden. Automated: the served HTML contains the switcher and summary elements and the fleet endpoints and no external URL.
- **AC-9:** Generated `docs/epic-status-api.md` and `docs/cli/` match the binary, and the Markdown–CLI drift gate passes for `plugins/epic-orchestrator`.
- **AC-10:** Given a hostile `ok` peer document with unknown fields, a 300-character phase, wrong-typed counts and a non-array `issues`, the machine entry contains only allow-listed fields, a 160-character phase, and empty or zero values for the wrong-typed ones; with no peers configured the document has only `local`.

## Acceptance evidence

All automated checks run real loopback TCP: in-process engines (`serve_with_http`, as `dashboard_live` does) and a small test HTTP peer that records each request head and sends a scripted reply. No mocking crate. Test-only helpers follow the crate's `support` / `herdr_fixture` precedent: `#[cfg(test)] #[path = "tests/peer_fixture_test.rs"] mod peer_fixture;` and `tests/fleet_rig_test.rs` (two-engine setup) wired the same way in `lib.rs`. AC tests are split so each file stays under 300 code lines: `fleet_test.rs` (AC-3, AC-10), `http_fleet_test.rs` (AC-1, AC-2, AC-4, AC-5), `http_fleet_proxy_test.rs` wired as a second test module of `http_fleet.rs` (AC-6, AC-8 static), `fleet_cli_test.rs` (AC-7). `http_test.rs` gains only the `peers` field in its `Config` literals.

| AC | Real input and expected observation | Runnable check |
|---|---|---|
| 1 | Two engines on loopback ports, registries, graphs, a `report` per engine; raw TCP GET on A | `cargo test -p toolu-epic-orchestrator fleet_two_engines` |
| 2 | Never-accepting `TcpListener`; engine B bound `0.0.0.0` with a status token and a wrong configured token; both requests timed | `cargo test -p toolu-epic-orchestrator fleet_slow_peers` |
| 3 | Fixture peers replying `schema: 2`, `text/plain`, `404`, `403`, `302`, a 1.5 MiB body | `cargo test -p toolu-epic-orchestrator fleet::tests` |
| 4 | Canaries in `TOOLU_EPIC_PEER_TOKENS`; fixture replies embed them; scan bodies, CLI outcome text and every file under A's root; read the fixture's heads | `cargo test -p toolu-epic-orchestrator fleet_token_canary` |
| 5 | Engine B stopped after an `ok` read; second read after 1.1 s; a listener with the refresher and no request; 5 concurrent requests against a fixture counting heads | `cargo test -p toolu-epic-orchestrator fleet_last_seen` |
| 6 | Proxied and rejected paths; 9 concurrent proxy calls against a blackhole | `cargo test -p toolu-epic-orchestrator fleet_proxy` |
| 7 | `query::status` with `--fleet` through a real `HOME` | `cargo test -p toolu-epic-orchestrator fleet_cli` |
| 8 | Automated static check; plus `docs/toolu/evidence/epic-fleet-dashboard.js` (Bun; the gate forbids tracked shell files), which starts two `target/debug/toolu epic serve` processes with temporary `HOME`/`TOOLU_RESOURCE_HOME`, registers a scripted epic in each, reports a phase, runs `"Google Chrome" --headless=new --disable-gpu --virtual-time-budget=6000 --timeout=8000 --dump-dom http://127.0.0.1:<A>/` (and `<B>`), saves `epic-fleet-dashboard.html`, and exits non-zero unless the A dump contains `local`, `server`, `never seen` and an unhidden `id="fleet-summary"`, and the B dump has the switcher hidden. `docs/toolu/` is gitignored, so the script and the saved dump are committed with `git add -f` | `cargo test -p toolu-epic-orchestrator fleet_dashboard`; the script is a manual macOS run with Chrome, recorded in the PR, not a CI check |
| 9 | Regenerate docs from the binary and run drift gates | `cargo test -p toolu-epic-orchestrator status_api_doc && cargo xtask docs-cli && cargo xtask check-markdown-cli` |
| 10 | Fixture peer with the hostile document; empty peer list | `cargo test -p toolu-epic-orchestrator fleet::tests` |

## Documentation impact

- `docs/epic-status-api.md`, generated from the `render()` text and endpoint table in `status_schema.rs`: the two fleet routes, states and reasons, and a **Fleet over a private network** section — peers in `epic.peers`, tokens in `peer_tokens` / `TOOLU_EPIC_PEER_TOKENS`, the URL naming the peer's bound IP and port (or a TLS proxy forwarding that `Host`), plain HTTP only on a private network, and that a status server must not face the public internet without its token and TLS in front.
- `docs/config.md`: the peer example becomes `http://192.168.1.20:7717`, plus the Host rule and a link to that section.
- `plugins/epic-orchestrator/commands/epic.md`, `skills/epic-orchestrator/SKILL.md`, `plugins/epic-orchestrator/README.md`: `toolu epic status --fleet`.
- `AGENTS.md` Key files row for `crates/epic-orchestrator/src/lib.rs`: the fleet modules.
- `docs/cli/` regenerated; the `commands --json` snapshot accepted.

## Open Questions

None blocking. Naming the local machine from `HOSTNAME`/`HOST` was rejected: macOS shells do not export them, so the name would differ by launch path; owner this issue, revisit on request.

## Review

Three adversarial review rounds (2026-10-13). Round 1: one blocker (peer `Host` rule vs TLS in front) and should-fixes; round 2: should-fixes; round 3: the round-time definition and the force-add note, both applied above. Everything else was reported resolved.
