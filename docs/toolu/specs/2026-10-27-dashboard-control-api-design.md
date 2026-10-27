# Dashboard control API — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Falconiere Barbosa (epic #649 worker)   **Topic:** Token-gated `POST /api/v1/actions/{op}` on the engine's status listener (#650)

## Problem

The dashboard can only read. Pausing, resuming, acknowledging, answering a worker or releasing a hold needs the CLI. Wave 2 to 4 of epic #649 (the control UI in #656) need a safe HTTP write path. That path must keep the state thread as the only writer. It must also refuse cross-origin and unauthenticated requests, even on loopback, because `answer` types text into agents that run with approval bypass.

## Non-Goals

1. No control of fleet peers. The peer proxy stays GET-only.
2. No host pause or resume (#653), screen reads (#654) or UI (#656).
3. No CORS headers, no retries, no TLS, no async runtime.
4. A token from `TOOLU_EPIC_TOKEN` or `TOOLU_EPIC_STATUS_TOKEN` cannot rotate without a restart.

## Architecture

**Request parsing (`http_request.rs`).**
- `Request` gains `origin: Option<String>`, `content_type: Option<String>` and `body: Vec<u8>`.
- `Content-Length` is recorded instead of refused.
- After the headers:
  - a `POST` without `Content-Length` is `ReadError::Bad` (400);
  - a `POST` whose length is above 16 KiB is `ReadError::TooLarge` (413);
  - otherwise exactly that many body bytes are read.
- Any other method with a length above 0 is still 413.
- `Transfer-Encoding` stays 400.

**Routing (`http.rs`, new `http_action.rs`).**
- `respond` sends every path under `/api/v1/actions/` to `http_action::respond` after the existing host check.
- Every other path keeps today's behaviour exactly. That includes a `POST` to them: `405` with `Allow: GET, HEAD`.
- `Response` gains an `allow: &'static str` field. The 405 that `http_action` sends for a non-`POST` sets `"POST"`.

The action gate runs in this order and stops at the first failure:

| # | Check | Failure |
| --- | --- | --- |
| 1 | Method is `POST` | `405 method_not_allowed`, `Allow: POST` |
| 2 | A status token is configured | `403 control_disabled` |
| 3 | `http_access::authorized` (constant time) | `401 unauthorized` |
| 4 | `Origin` equals `http://` + the request's `Host` (which is already validated) | `403 foreign_origin`; a missing `Origin` also fails |
| 5 | The `Content-Type` media type, case-insensitive with parameters ignored, is `application/json` | `415 unsupported_media_type` |
| 6 | The op is in `pause`, `resume`, `ack`, `answer`, `release` | `404 unknown_op` |
| 7 | The body is a JSON object and its fields are valid | `400 invalid_request` |

Field rules:
- Keys, epics and kinds match `[A-Za-z0-9._-]{1,128}`.
- `pause` and `resume` take an optional `epic`.
- `ack` takes exactly one of `key` or `kind`; either is forwarded as the engine `key`. The engine already treats a key equal to a keyless item's kind as that item.
- `answer` takes `key` and `text`. The text is 1 to 4,000 characters, and its only allowed control characters are `\n` and `\t`.
- `release` takes `key`.
- Unknown body fields are ignored.

**Forwarding.**
- `http::Config` gains `control: Option<Sender<Msg>>`. `socket::start_http` sets it to a clone of the state loop's sender.
- The handler builds `{"op", ...fields, "by": "web"}` and sends `Msg::Line { request, reply }`. It then waits with `recv_timeout(35 s)`. The deadline outlasts herdr's 30 s prompt timeout, so a slow `answer` gets its own `uncertain` reply rather than a `503` and a retry that types it twice (code review, 2026-10-27).
- A timeout, a closed channel or a missing sender returns `503 engine_busy`, with no retry.
- A reply object with `error` returns `409` with body `{"schema":1,"error":<engine text>}`. Any other reply returns `200` with the reply body.
- HTTP threads never touch `World` for an action.

**Engine changes (shared with the CLI).**
- `Engine::pause` and `resume` take a `by`. They push the journal record `action pause|resume`: the key is the epic, or empty for every epic, and the note is `<epic|all> by=<by>`. They then flush.
- `dispatch` reads an optional `by` for `pause`, `resume` and `release`, defaulting to `user`.
- `holds::release` takes a `by`, and its journal note becomes `<with> by=<by>`.
- `reply_answer`, when `send_prompt` fails with an uncertain error, replies `{"ok": true, "delivered": "uncertain", "note": "worker prompt may not have arrived: <err>"}`. It journals `action answer-uncertain` for the key with note `by=<by>` and leaves attention open.
  - Uncertain means the error text contains `: timeout:`, `: agent_prompt_stalled:` or `timed out`. These are the herdr codes that `herdr_calls` marks `uncertain`, plus a socket read deadline.
  - A scripted `prompt` (the `TOOLU_EPIC_SCRIPTS` effect seam) that prints `{"outcome":"uncertain"}` is uncertain too (`prompt uncertain`). Today that outcome counts as success.
  - A non-uncertain failure keeps today's error reply.

**Secrets reload (`http_secrets.rs`).**
- `Context.secrets` becomes a `LiveSecrets` holding `Mutex<State>`. The state has `Arc<Secrets>`, the last-seen file stamp (`Option<SystemTime>`) and the last check `Instant`.
- `current()` re-stats `secrets::path(roots)` at most once a second. When the stamp changed, it calls `secrets::load`. On error it keeps the previous value.
- `Config.reload: Option<Roots>` is set by `control::http_config`. When it is `None`, as in tests, the secrets are fixed.
- Every request uses `current()`. The fleet cache keeps the peer tokens it was built with.

**Bound address.**
- After `http::start` binds, `socket::serve_with_http` writes `Paths::http()` = `<root>/http.json` with `{"addr": "<bound>"}`, using `disk::write_value`.
- `join_threads` removes the file after stopping the listener.
- `http.json` holds no secret.

**Token URL (`toolu epic token url`).**
- If no engine holds `engine.lock`, the command fails with `toolu epic token url: no engine is running`.
- If there is no `http.json`, it fails with `toolu epic token url: the engine has no status listener`.
- If the token is empty, it fails with `toolu epic token url: no status token; run toolu epic token new`.
- Otherwise it prints `<notify_notice::dashboard(addr)>#token=<token>`. A `0.0.0.0` or `::` bind prints `127.0.0.1`.

## Interfaces / Schema

```
POST /api/v1/actions/pause    {"epic"?: KEY}
POST /api/v1/actions/resume   {"epic"?: KEY}
POST /api/v1/actions/ack      {"key": KEY} | {"kind": KEY}
POST /api/v1/actions/answer   {"key": KEY, "text": TEXT}
POST /api/v1/actions/release  {"key": KEY}
Headers: Authorization: Bearer <token>; Origin: http://<Host>; Content-Type: application/json; Content-Length ≤ 16384
200 <engine reply>   409 {"schema":1,"error":"<engine text>"}   400/401/403/404/405/413/415/503 {"schema":1,"error":"<code>"}
```

- **Journal:** `action pause` and `action resume` (note `<epic|all> by=<by>`); `action release` (note `<with> by=<by>`); `action answer-uncertain` (note `by=<by>`). The existing `ack` and `answer` judgment records carry `by=web` for web requests.
- **Files:** `<resource root>/http.json` holds `{"addr":"127.0.0.1:7717"}`.
- **CLI:** `toolu epic token url` (no flags).

## Failure modes and edge cases

- **Socket-level faults:**
  - A body shorter than `Content-Length` hits the 5 s read timeout and the connection closes without a reply. This is today's I/O-error path.
  - A body that is not UTF-8 or not JSON returns 400.
  - A body of `null` or an array returns 400.
- **A busy state thread:**
  - A slow `answer` (herdr) or effect replay can keep the thread past 10 s. The handler then returns 503 and drops its receiver.
  - The engine may still apply the action later. Its `reply.send` fails silently and the journal records it.
  - The 503 body says only `engine_busy`; the docs say the action may still apply.
- **Draining engine:** after `stop` the receiver is dropped, `send` fails and the handler returns 503.
- **Token rotation:**
  - Within 1 s of rotation, the old token may still pass.
  - Deleting `secrets.json` disables control within 1 s, unless an environment token is set.
  - A malformed or unsafe `secrets.json` keeps the last good secrets.
- **Token leakage:**
  - The token is never echoed. Error bodies are fixed codes. Engine replies hold no secrets.
  - The journal never sees the `Authorization` header, because `by` is the fixed `web`.
- **Loopback aliases:** `Host: localhost:7717` with `Origin: http://localhost:7717` passes. `Host: 127.0.0.1:7717` with `Origin: http://localhost:7717` fails (`foreign_origin`).
- **Keep-alive:** a refused action closes the connection (`Response::error` sets close). A successful one honours `Connection`.

## Acceptance criteria

- **AC-1:** A real engine runs on loopback with a token. `POST /api/v1/actions/pause` with the token, a same-origin `Origin` and JSON returns 200 `{"ok":true}`. `/api/v1/overview` then shows `"paused": true`. `/api/v1/events/stream` emits an `action`/`pause` event whose note ends `by=web`, and `toolu epic pause` (socket op without `by`) writes the same record with `by=user`.
- **AC-2:** These refusals each return their status and leave the journal unchanged: no `Authorization` (401), a wrong token (401), a foreign `Origin` (403 `foreign_origin`), `text/plain` (415), no token configured (403 `control_disabled`). A `POST` without `Content-Length` returns 400; a body over 16 KiB returns 413; a `GET` on an action path returns 405 with `Allow: POST`.
- **AC-3:** `resume`, `ack` (by key and by kind), `answer` and `release` over HTTP give the same reply as the same socket op, apart from `by`. `release` of a key that is not held returns 409 with `<key> is not held`, matching the socket reply. An unknown op returns 404.
- **AC-4:** `answer` returns 400 for empty text, for 4,001 characters, and for text containing `\u0007`; text with `\n` and `\t` passes validation. A valid answer for a key with no live agent returns 409 `no live agent for <key>`. A prompt whose delivery is uncertain (herdr `timeout`/`agent_prompt_stalled`, or scripted `uncertain`) returns 200 `"delivered":"uncertain"` and leaves the attention item open.
- **AC-5:** After AC-1 to AC-4 run, the token appears in no response body, no journal record and no file under the resource root, `http.json` included.
- **AC-6:** `toolu epic token url` against a running engine prints `http://127.0.0.1:<port>/#token=<token>`. With no engine it fails with `no engine is running`; with no token it fails with `no status token`.
- **AC-7:** The listener re-reads a rotated `secrets.json`: after `secrets::rotate_status_token`, the old token gets 401 and the new token gets 200 (after the 1 s check window).
- **AC-8:** A real engine whose state thread is busy (a scripted `prompt` that sleeps 1 s while it handles an `answer`) answers a second action with 503 `engine_busy` once the deadline passes, and the server never resends it. The deadline is `Config.deadline`: 35 s in the engine, 200 ms in the test.
- **AC-9:** Every existing GET route and status test passes unchanged. `cargo xtask gate`, `cargo xtask check-cli-compat` and `cargo xtask check-markdown-cli` pass.
- **AC-10:** A live `answer` from the HTTP route reaches a real herdr worker. The probe runs a built engine with a temporary resource home against the real herdr server and records the result in `docs/toolu/evidence/` and the PR body.

## Acceptance evidence

| AC | Real input | Expected | Check |
| --- | --- | --- | --- |
| AC-1 | `serve_with_http` engine thread, a scripted epic, `Secrets` with a token, TCP requests | 200, overview `paused:true`, SSE event `pause` with `by=web`; socket pause gives `by=user` | `cargo test -p toolu-epic-orchestrator http_action` |
| AC-2 | The same engine; raw requests per refusal | Status codes; journal line count unchanged | same |
| AC-3 | Engine with a held issue and an attention item; HTTP versus `exchange_retry` replies | Equal replies; 404 for `nope` | same |
| AC-4 | Engine with a scripts dir whose `prompt` prints `{"outcome":"uncertain"}`, plus the classifier on real herdr error JSON (`timeout`, `agent_prompt_stalled`) parsed by `herdr_calls::delivered` | 400s; 409 no live agent; 200 uncertain | same plus `attention_ops` test |
| AC-5 | Token canary `CANARY-TOKEN-650` in-process; in the AC-6 binary test, the engine's captured stderr and stdout after an HTTP `pause` | No match in bodies, journal, resource root or engine output | same, plus `epic_token_url` |
| AC-6 | Built `toolu` binary, temporary `HOME`, `TOOLU_RESOURCE_HOME`, live engine (`epic engine --http 127.0.0.1:0`) | URL printed; failures named | `cargo test -p toolu-cli --test epic_token_url` |
| AC-7 | File-backed secrets via `Roots` with temporary `HOME` | 401 old, 200 new | `cargo test -p toolu-epic-orchestrator http_secrets` |
| AC-8 | Real engine thread, scripts dir with a sleeping `prompt`, 200 ms deadline | 503 `engine_busy`; the journal shows one action only | `cargo test -p toolu-epic-orchestrator http_action` |
| AC-9 | Whole workspace | Green | `cargo xtask gate`, `check-cli-compat`, `check-markdown-cli` |
| AC-10 | Built `toolu` engine, temporary `TOOLU_RESOURCE_HOME`, real herdr agent | Prompt text visible on the agent screen; journal `answered`/`answer` with `by=web` | Manual probe recorded in `docs/toolu/evidence/2026-10-27-dashboard-control-api-probe.md` |

## Documentation impact

- `docs/epic-status-api.md`: an "Actions" section covering the routes, authorization, error codes, the 10 s deadline, uncertain delivery, token rotation (and that an environment token needs a restart), `http.json` and `token url`.
- `docs/epic-orchestrator/README.md`: the dashboard paragraph covers control and `token url`.
- `docs/cli/`: regenerated with `cargo xtask docs-cli`, plus the CLI contract snapshot.


## Open Questions

- **The AC-10 probe needs a live herdr agent.** Owner: this worker. It does not block: the in-process tests prove the same code path through the scripted prompt seam. If no isolated agent can be started, the PR says so and records the closest real run.

Nothing else blocks. Decisions taken in the brainstorm (Jev-assisted) on uncertain answers, reload failure, and the `ack` `kind` field are recorded above.

## Spec review (2026-10-27)

- Acceptance criteria: 🟡 should-fix: the issue's test plan asks for a live herdr `answer` probe. Fixed: AC-10 added.
- AC-8: 🟡 should-fix: a test-held receiver does not represent a busy engine. Fixed: a real engine with a sleeping scripted prompt.
- AC-5: 🟡 should-fix: "log line" was not covered. Fixed: the engine output in the binary test is searched.
- Documentation impact: 🔵 consider: the docs should say an environment token needs a restart to rotate. Fixed.
- Jev (requirement coverage, evidence completeness) gave 0.65 and 0.65 before these fixes. After them, every issue requirement maps to a section or AC by manual cross-check.
