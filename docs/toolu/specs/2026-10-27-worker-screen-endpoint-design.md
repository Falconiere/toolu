# Worker screen endpoint — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Codex   **Topic:** token-gated local worker pane reads

## Problem

The dashboard cannot show the terminal of a worker that is currently working. Herdr's `recent` read fails with `agent_not_idle` for a working alternate-screen host. Operators currently have to attach to the pane to see it.

## Non-Goals

1. Reading or controlling a peer's worker screen.
2. Sending keys or prompts to a worker through this route.
3. Persisting terminal output or changing the engine's existing `Call::Read` tail used for other purposes.

## Architecture

Add `http_screen.rs` for one local GET route. Resolve `{key}` to `issue.agent` and `issue.agent_status` in `World`, then call herdr `agent.read` over the protocol-22 socket using `source: visible`, `format: text`, and **no** `lines` parameter. Reuse the connection timeout, error-code handling and shared eight-call limiter from `http_live.rs`; never use `Client`'s CLI fallback. Cache raw read results per issue key and agent target for one second. Redact with the current `LiveSecrets` value when producing every response, then trim the beginning until the serialized JSON body is at most 65,536 bytes. Preserve valid UTF-8 and the final line. This keeps a changed configured secret from being exposed by an old redacted cache entry.

The route is intercepted before the loopback read-auth exception. It uses `http_access::authorized` on every request, including loopback and HEAD. Use #650's `403 control_disabled` when no token is configured and `401 unauthorized` for a missing or wrong token. The normal host validation still runs first. Keep `http_fleet`'s allow-list unchanged. The blocked-agent attach text is formed from `issue.agent` and the engine's optional herdr session name, shell-quoted for copy/paste.

## Interfaces / Schema

`GET /api/v1/issues/{key}/screen` returns `application/json`, `Cache-Control: no-store`, and `schema: 1`:

```json
{"schema":1,"key":"falconiere-toolu-654","agent_status":"working","text":"visible pane text","lines":1,"sampled_at":"2026-10-27T00:00:00.000Z"}
```

`lines` counts `text.lines()` after redaction and truncation; empty text has zero lines. A blocked result adds `"attach": "herdr agent attach <target>"`, with `herdr --session <name>` before `agent` when configured. `sampled_at` records the herdr read time and is identical on cache hits. An absent agent or pane returns `200` with `{"schema":1,"key":KEY,"state":"not_running"}`. An off, scripted, disconnected or protocol-mismatched herdr returns `200` with `{"schema":1,"key":KEY,"available":false,"reason":REASON}`. Reasons follow #651: `off`, `scripted`, `not running`, `protocol <n>`, or `error: <code>`. A full limiter returns `503 {"schema":1,"error":"herdr_busy"}`. Unknown or invalid issue keys return `404` after authorization. HEAD follows GET authentication and status but has no body.

## Failure modes and edge cases

- Missing token configuration: `403 control_disabled`; missing or wrong bearer with a configured token: `401`, even on loopback. No herdr call or issue lookup follows either refusal.
- Herdr `agent_not_found` or `pane_not_found`: `not_running`, never a 5xx. Other herdr error objects and transport/timeouts: `available:false`, with a bounded reason. A newer protocol is unavailable with no CLI fallback.
- Herdr is off, scripted or stopped: `available:false` without waiting on the state thread. An empty successful screen has `text:""` and `lines:0`.
- Concurrent same-key reads share one cache fill. Each read that needs herdr acquires #651's limiter; a ninth overlapping request can get `503 herdr_busy`. Separate keys have separate one-second entries.
- Redaction precedes truncation. Truncation works at UTF-8 boundaries and counts the JSON escape overhead, so the whole response body stays within 65,536 bytes. The last line is retained; if that line alone exceeds the cap, its suffix is retained.
- The attach command is emitted only for `agent_status:blocked`, uses the mapped herdr agent target, and quotes an unsafe shell word. It is text, never executed by the server.

## Acceptance criteria

- **AC-1:** A real working Claude Code worker and a real working Codex worker each return `200` with the current visible pane text; herdr `recent` is never called.
- **AC-2:** Without the configured bearer token, a loopback request returns `401` and makes no herdr call. With no configured token it returns `403 control_disabled`.
- **AC-3:** A configured fake secret printed by a real pane appears only as `<redacted>` in the screen response. Cached raw text is redacted with the current secrets on each response.
- **AC-4:** A real pane with more than 64 KiB of output returns a JSON body of at most 65,536 bytes, trims the top, retains its last line and valid UTF-8, and reports the displayed line count.
- **AC-5:** A finished worker or closed pane gives `200 state:not_running`; herdr off or unavailable gives `200 available:false` with a reason, without a 5xx.
- **AC-6:** A blocked worker returns its visible dialog, `agent_status:blocked`, and a session-aware ready-to-copy attach command.
- **AC-7:** `/api/v1/fleet/peers/{name}/issues/{key}/screen` returns `404` and makes no peer request for that path.
- **AC-8:** Two same-key reads inside one second return the same `sampled_at` after one herdr read; above the shared eight-call limit a request gets `503 herdr_busy` while `/overview` remains responsive.
- **AC-9:** `cargo xtask gate` passes.

## Acceptance evidence

| AC | Representative real input / boundary | Expected observation | Runnable check |
| --- | --- | --- | --- |
| 1 | Isolated herdr 0.9.5 session; engine-launched Claude Code and Codex workers during a long command | Visible text and no `agent_not_idle`; capture herdr request params | Live probe following `.toolu/skills/verify-epic-watcher/SKILL.md`; focused Rust socket test |
| 2 | Loopback listener with a configured token, then no token | `401`, then `403`; herdr call count unchanged | Real HTTP listener test |
| 3 | Pane prints a configured canary; a render test passes two secrets sets over the same cached sample | Canary absent and `<redacted>` present; cached text uses the current secrets | Real herdr pane probe plus Rust render test |
| 4 | Pane prints a fixture over 64 KiB into an isolated headless viewport configured with `headless_cols = 800` and `headless_rows = 100`, ending with a unique Unicode line; a socket test checks the exact byte boundary | Body byte count at most 65,536, prefix removed, final line intact, `lines` matches text | Real herdr pane probe plus focused Rust socket test |
| 5 | Close a real pane; separately run herdr off and stop its server | `not_running` or `available:false`, all `200` | Live probe plus real listener test |
| 6 | Real socket fixture with a blocked worker status, visible dialog, and named session | Dialog, `blocked`, `herdr --session <name> agent attach <target>` | Real listener test; live trust dialog not reproduced |
| 7 | Configured peer with a screen URL | `404`, peer call count unchanged | Existing real peer HTTP fixture test |
| 8 | Two immediate reads; nine overlapping stalled herdr reads, then `/overview` | One sample, at least one `503`, responsive overview | Real socket/listener concurrency test |
| 9 | Full repository | All gate steps green | Native `cargo xtask gate` through ledger step s4 |

## Documentation impact

Add the endpoint table entry and its authentication, response states, caching and truncation behavior to the generated `docs/epic-status-api.md` through `status_schema.rs`. Record live probe evidence under `docs/toolu/evidence/`.

## Open Questions

None. The issue does not define `lines`; this spec defines it as the count of returned text lines because herdr returns text without a line count. The issue says payload, so the 64 KiB limit applies to the serialized JSON body.
