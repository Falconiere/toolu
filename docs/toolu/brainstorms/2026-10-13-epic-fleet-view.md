# Epic fleet view — Brainstorm

**Date:** 2026-10-13   **Issue:** #453   **Path:** Full (public interface, security)

## Capsule

- **Outcome:** one local status server (and `toolu epic status --fleet`) lists this
  machine's engine beside every configured peer engine, with a per-peer state and
  age, a fleet summary, and peer detail pages proxied through the local server.
- **Material defaults / non-goals:** read-only GET of each peer's existing
  `/api/v1/overview` and detail routes; no peer event-stream proxy, no peer
  discovery, no TLS termination in the engine, no persisted last-seen times.
- **Repository evidence:** `epic.peers` (`toolu_runtime::config::epic::Peer`) and
  `Secrets::peer_token` already exist from #463; the listener (`http.rs`) serves
  allow-listed documents from per-connection threads; `toolu-http` is the only
  HTTP client crate; `logic.rs` defines a live issue as stage `running` or
  `awaiting_merge`.
- **Risk:** a peer accepts only its bound `Host`, so a peer URL must name the
  peer's bound IP and port (or a TLS proxy in front). `http.rs` and `verbs.rs` sit
  near the 300-code-line limit, so new code goes in new modules.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Where the CLI gets peer data | The CLI process reads the local overview over the control socket and fetches peers itself with the shared fleet functions | The control socket is served by the single state thread; a 2 s network wait there stalls the engine. Works with `--no-http`. Jev: `cli_fetches` 0.99 | Two call sites share one function; covered by both tests |
| Peer data in `/api/v1/fleet` | Allow-listed projection, then redaction with local secrets | Peers are semi-trusted; #449 assembles every response from allow-listed fields. Jev: `allow_list` 1.0 | A future peer field needs a projection change |
| Peer `403` (foreign Host) | `unauthorized` | Only four states exist; 403 is an access refusal the user fixes in config. Jev: `unauthorized` 0.75 | Docs must name the Host/port cause |
| In-flight issues | Stage `running` or `awaiting_merge` | `logic.rs` `live`. Jev: `live` 1.0 | None |
| Peer fetch timing | Parallel scoped threads, 2 s whole-request deadline, 1 s result cache shared by all requests | Issue asks for parallel and short timeout; the dashboard refreshes on every local event | A blackholed peer delays only the fleet document, never local routes |
| Proxy | `/api/v1/fleet/peers/{name}/{route}` for the allow-listed read routes, segment charset checked | Issue: the browser needs only the local address | Peer event streams are not proxied; peer view polls every 5 s |

## Alternatives rejected

- **Engine control-socket op for `--fleet`:** blocks the state thread on network I/O.
- **CLI calls its own `/api/v1/fleet`:** fails with `--no-http` and needs the local token on a non-loopback bind.
- **Embedding whole peer overviews:** forwards unreviewed peer fields (worktree paths, leases) and any future field.
- **Background poller thread per peer:** constant traffic with nobody watching; the request-driven cache gives the same freshness when a page is open.
