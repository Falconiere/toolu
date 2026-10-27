# Dashboard control API (#650): brainstorm

Delivery mode, Full path. The change adds a public HTTP interface and has security and persistence effects.

## Capsule

- **Outcome:** On the engine's own status listener, `POST /api/v1/actions/{pause,resume,ack,answer,release}` does what the matching CLI verb does. Each request needs the status bearer token, a same-origin `Origin` and `application/json`. `toolu epic token url` prints the token URL for the running engine.
- **Material defaults and non-goals:**
  - Control applies to the local engine only. The peer proxy stays GET-only.
  - There are no host pause routes (#653) and no screen reads (#654).
  - The server sends no CORS headers and never retries.
- **Repository evidence:**
  - comemory `38cad20a` recorded that web control is local-engine only, with a token on every mutation.
  - The issue's "Review additions" section names the in-process `Sender<Msg>`, `http.json`, the mtime reload of secrets and `pause`/`resume` journaling.
  - Code involved: `http.rs`, `http_request.rs`, `http_access.rs`, `socket.rs`, `socket_loop.rs` (`Msg::Line`), `dispatch.rs`, `attention_ops.rs`, `holds.rs`, `server.rs` (`pause`/`resume`), `verbs.rs` (`token new`), `notify_notice::dashboard`.
- **Risk:**
  - `answer` types text into agents that run with approval bypass, so every gate must fail closed.
  - An uncertain-delivery reply changes the CLI's `answer` result: it was exit 1 and becomes exit 0 with `delivered: "uncertain"`.
  - The `release` journal note gains ` by=<who>`.
- **Handoff:** spec.

## Axes

| Axis | Decision | Evidence |
| --- | --- | --- |
| Write path | Clone the state loop's `Sender<Msg>` into the HTTP context and wait on `recv_timeout(10 s)`. On timeout, return `503 engine_busy` and never retry. | Issue review additions. `client::exchange` has no read timeout and spools on a protocol mismatch. |
| Gate order | 1. Host check (existing). 2. Method `POST` (`405`, `Allow: POST`). 3. Control enabled (`403 control_disabled`). 4. Bearer (`401`). 5. Origin equals `http://` + Host (`403 foreign_origin`). 6. Content type (`415`). 7. Op allow-list (`404`). 8. Body and fields (`400`). | Unauthenticated callers learn nothing about which ops exist. |
| Uncertain answer | The engine replies `{"ok":true,"delivered":"uncertain","note":…}` and journals `action answer-uncertain` with `by`. It leaves attention open. | Jev chose `ok_open` (p = 0.97) over settling, or keeping an error with a flag. An answer that may not have arrived must stay visible. |
| Secrets reload | Stat `secrets.json` at most once a second. Reload when the mtime or the file's existence changes. A failed load keeps the previous secrets and retries. | Jev chose `keep_previous` (p = 0.85). Redaction of GET responses must not drop on a transient error. |
| Web `ack` | The body has exactly one of `key` or `kind`, forwarded as the engine `key`. Both or neither returns `400`. | The engine `ack` already treats a key that equals a keyless item's kind as that item. Jev chose `either` (p = 0.54); repository semantics agree. |
| Bound address | The engine writes `http.json` `{"addr": "<bound>"}` under the resource root after binding and removes it on stop. | Issue review additions. |
| Token URL | `toolu epic token url` prints `http://<host>:<port>/#token=<token>`, with the host chosen as `notify_notice::dashboard` does. | Issue review additions. |

## Rejected

- **Unix-socket round trip from HTTP threads.** It has no deadline and can trigger a replace.
- **Fail closed on a secrets reload error.** It drops redaction values from GET responses.
- **Settling attention on an uncertain answer.** It hides a possibly lost answer.
