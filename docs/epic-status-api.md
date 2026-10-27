# Epic status API

Generated from `crates/epic-orchestrator/src/status_schema.rs`. Regenerate with `TOOLU_UPDATE_DOCS=1 cargo test -p toolu-epic-orchestrator status_api_doc`.

The resident engine serves HTTP/1.1 on `127.0.0.1:7717` by default. `toolu epic serve --http ADDR` selects another address; `--no-http` disables the listener. Non-loopback binds require a status token from `toolu/secrets.json` or `TOOLU_EPIC_STATUS_TOKEN` / `TOOLU_EPIC_TOKEN`, sent as `Authorization: Bearer <token>`.

Read routes accept GET and HEAD; actions accept POST. HEAD has no body. Every JSON body contains `schema: 1`. HTML and JSON responses have `Cache-Control: no-store`; versioned dashboard assets have `Cache-Control: public, max-age=31536000, immutable`. Every response carries `X-Content-Type-Options: nosniff` and a same-origin Content-Security-Policy. No response has a CORS header. Errors use fixed JSON codes. The listener accepts only its bound Host and loopback aliases.

| Path | Purpose | Fields after `schema` |
|---|---|---|
| `/healthz` | Engine health | version, uptime_ms, journal_seq |
| `/api/v1/overview` | Current aggregate | engine, paused, hostname, journal_seq, epics, issues, attention, counts, attention_count, hosts, machine, jobs |
| `/api/v1/epics` | Registered epics | items (key, paused, counts, attention_count, title, url, waves_total, current_wave, live, queued) |
| `/api/v1/epics/{id}` | One epic board | key, paused, title, url, waves_total, current_wave, live, queued, graph, issues (key, stage, phase, pr, host, agent_status, agent_status_at, model, effort, phase_age_ms, hold, plan_summary), queue |
| `/api/v1/issues/{key}` | One worker | key, epic, stage, phase, phase_age_ms, agent_status, agent_status_at, host, model, effort, pr, last_gate_verdict, queue_position, launches, phase_history, recent_events, hold, plan |
| `/api/v1/issues/{key}/screen` | One worker's visible terminal screen | key, agent_status, text, lines, sampled_at; attach when blocked; or state, or available and reason |
| `/api/v1/issues/{key}/logs` | One local worker's background logs | items (id, source, label, state, size, started_at, updated_at); or available and reason |
| `/api/v1/issues/{key}/logs/{id}` | One background log's bounded tail or next page | text, offset, next_offset, size, state, truncated; or available and reason |
| `/api/v1/agents` | Worker sessions | items (state, key, epic, worktree, session_id) |
| `/api/v1/agents/live` | What each worker's agent is doing now | available, reason, sampled_at, items (key, epic, host, agent_status, title, sampled_at) |
| `/api/v1/hosts` | Host capacity | items (host, cap, running, cooldown_until, reason, paused, paused_by, paused_at) |
| `/api/v1/machine` | Shared resources | load, available_memory_bytes, steal, pressure, policy, agent_leases, job_leases |
| `/api/v1/jobs` | Expensive jobs | running, waiting |
| `/api/v1/queue` | Ready work by repository | repositories (repo, items) |
| `/api/v1/attention` | Live attention | items (kind, key, epic, note, seq, state, age_ms) |
| `/api/v1/metrics` | Epic run metrics | epic, since, generated_at, records, issues, lead_time_ms, phases, ready_to_merged_ms, rebase_rounds, prompts, judgments, wakeups, github, jobs, launches, engine |
| `/api/v1/fleet` | This machine and every peer | summary (machines, reachable, hosts, in_flight, attention_count), machines (name, local, state, reason, hostname, dashboard_url, age_ms, last_seen, engine, paused, journal_seq, epics, in_flight, hosts, counts, attention_count) |
| `/api/v1/fleet/peers/{name}/{route}` | One peer's document, read through this server | the peer's document for route overview, epics, epics/{id}, issues/{key}, agents, hosts, machine, jobs, queue or attention |
| `/api/v1/events` | Journal replay | events, oldest_seq, next_seq |
| `/api/v1/events/stream` | Live journal feed | SSE id, event, data |

`/api/v1/events` accepts `since` (default 0) and `limit` (default 100, maximum 500). An event contains `seq`, `at`, `kind`, `name`, `epic`, `key`, and a bounded `note`. The stream emits `id: <seq>`, `event: journal`, and `data: <JSON>`; `Last-Event-ID` overrides `since`. A cursor older than retained history receives a `reset` event.

`/api/v1/metrics` accepts `epic` and `since` (`YYYY-MM-DD` or an ISO-8601 UTC time) and serves the document of `toolu epic metrics --json` (`$defs/epicMetrics` of `toolu commands --schema`), recomputed at most once a minute per query.

## Board header and worker rows

Each item of `/api/v1/epics`, the overview's `epics` and `/api/v1/epics/{id}` carry the board header: `title` and `url` from `graph.json` `epic` (`null` when empty or when the engine has no parsed graph), `waves_total`, `current_wave`, `live` and `queued`. Waves are stable for the run: the engine levels every graph issue, open or closed, as 1 + its deepest blocker in the epic, so a merged issue keeps its wave (the graph's own `wave` levels only open issues and renumbers after each refresh). The epic document's `graph.waves` and `graph.issues[].wave` use the same stable numbering whenever the engine has parsed the graph. They are computed once each time the engine loads the graph. An issue is done once its pull request merged (stage `merged`, or still `cleaning` or `cleanup-incomplete`), when it is `abandoned`, or when it was closed before the engine launched it. `current_wave` is the lowest wave that still holds an issue that is not done, or `null` when every issue is done; a dependency cycle among open issues gives no waves, and one through a closed issue falls back to the open issues' waves. `live` counts issues whose stage is `running` or `awaiting_merge`, and `queued` those never launched (empty stage, no launch in flight) and not done.

Each issue row on `/api/v1/epics/{id}` adds `model` and `effort` (the route record, else the launch record), `phase_age_ms` and `hold`, all equal to the same fields of `/api/v1/issues/{key}`. `hold` is `{"kind", "with", "at"}` for a coupling hold, else `null`; its `score` and `source` are never exposed.

`hostname` on `/api/v1/overview` is this machine's host name up to the first `.`, or `null` when it cannot be read. On `/api/v1/fleet` every machine carries the `hostname` of its own overview (`null` for a machine that is not `ok` or reports an empty name) and `dashboard_url`: `null` for this machine, and for a peer its configured `url`. Config refuses a peer URL with a user name, password, path, query or fragment, so `dashboard_url` is always a bare origin such as `http://10.0.0.2:7717/`.

Configured secrets are redacted in every field. `title`, the `hold` strings and a peer's `hostname` are cut to 160 characters, after redaction, so a secret is never cut in two. A `url` longer than 160 characters is `null` rather than a broken link.

## Worker plans

`plan` on `/api/v1/issues/{key}` (and in `toolu epic status --json <key>`, which builds the same document) is the worker's plan ledger, the todo list `toolu ledger` keeps. It holds `plan_doc`, `updated_at`, `next`, `summary` (`total`, `green`, `red`, `pending`, `running`, `stale`) and `steps[]` (`id`, `title`, `status`, `activity`, `last_run`). Nothing else from the ledger is exposed: no `check`, `evidence_tail`, `diff_sha`, `scope_sha`, `input` or `retries`. Every string is cut to 160 characters, `steps` lists at most the first 200 steps (`summary` still counts all of them), and configured secrets are redacted. A step's `status` is the one the ledger stores (`pending`, `running`, `green`, `red`); the engine does not recompute freshness, so `summary.stale` is the count from the worker's last ledger write.

The engine reads the ledger on every request from the worker's worktree, for the host it was launched on (both from its launch record, else the engine's copy of it), and for the branch named by the worktree's `.git` and `HEAD` files. It reads those files directly and runs no git, so a branch switch shows the new branch's ledger at once. The ledger directory depends on the host: Claude Code `.claude/tmp/plan-ledger/`, Codex `.codex/tmp/plan-ledger/`, Cursor `.cursor/tmp/plan-ledger/`, and Muse `.toolu/tmp/muse/plan-ledger/`. `TOOLU_PROJECT_CONFIG_DIRNAME` in the engine's environment overrides the directory as it does for the worker. `plan` is `null` when there is no worktree, no known host, no `HEAD`, a reftable repository (whose `HEAD` names no branch without git) or no ledger yet, for example while the worker is still in its spec phase. A ledger that is empty, is not JSON or has a `version` other than 1 gives `{"error": "unreadable"}`, and the rest of the document is unaffected.

Each issue on `/api/v1/epics/{id}` carries `plan_summary`: the same counts plus `next`, `null`, or `{"error": "unreadable"}`. It is cached by ledger path, modification time and inode, so a board refresh rereads only ledgers that changed.

## Live agent status

Each issue keeps herdr's last `agent_status` (`idle`, `working`, `blocked`, `done` or `unknown`) and `agent_status_at`, the ISO time it last changed (`null` before any change). The engine's herdr subscription sets them: a `session.snapshot` seeds every issue (an agent herdr does not list is `unknown`), each `pane.agent_status_changed` updates one in the same step that journals its `herdr-event` row, a closed or exited pane sets its issue to `unknown`, and a herdr outage sets every issue to `unknown`. `/api/v1/issues/{key}` and the `issues` rows of `/api/v1/epics/{id}` carry both fields, so a page that follows `/api/v1/events/stream` sees a change without calling herdr.

`/api/v1/agents/live` adds each worker agent's terminal title and is the status source while the subscription is down. It lists one item per issue whose herdr agent `name` equals the issue's agent name; herdr agents that map to no worker are never listed. `title` is herdr's `terminal_title_stripped` with configured secrets redacted, then cut to 160 characters, or `null`. The items come from one `agent.list` call over the herdr socket, cached for 2 s and shared by every request; `sampled_at` is the time of that call. Only HTTP threads call herdr, each on its own connection with a 1 s connect, read and write timeout, and at most 8 herdr calls from HTTP run at once: a request that needs herdr while every slot is taken gets `503 herdr_busy`. When herdr cannot be read, the route answers `200` with `available: false`, a `reason` and no items: `off` (`TOOLU_EPIC_HERDR=off`), `scripted` (`TOOLU_EPIC_SCRIPTS`), `not running` (no socket or no reply: herdr is not installed or its server is down), `protocol <n>` (a herdr protocol other than 22; no CLI is run) or `error: <code>` (herdr answered an error). The route follows the other routes' authorization and is never proxied to a peer.

### Host probe

Observed on 2026-10-27 with herdr 0.9.5 through `herdr agent list` during real sessions:

| Host | `agent_status` values seen through a turn | Terminal title |
|---|---|---|
| Claude Code 2.1.311 | `blocked` on the folder-trust dialog at startup; through a turn `idle` → `working` → `idle` (it never reports `done`) | `Claude Code` until the first prompt, then a live task summary ("Notes.txt file listing") that stays after the turn. A worker the engine starts gets `-n <key>`, and its title is then the key itself ("falconiere-toolu-650"), not a summary. |
| Codex 0.164.0 | `blocked` on the folder-trust dialog at startup; `done` → `working` → `done` | The folder name ("ws-codex") until the first prompt, then `<task summary> \| <folder>` ("List files and save notes \| ws-codex"). |
| Cursor Agent 2026.10.22 | `idle` → `working` → `done` | `Cursor Agent` until the first prompt, then a task summary ("List Files And Write"). |
| Muse Code 1.4.13 | `idle` → `working` → `done` | The folder name ("ws-muse") throughout; no task summary. |

## Worker screens

`GET /api/v1/issues/{key}/screen` shows this engine's worker pane. It always needs `Authorization: Bearer <status token>`, even on loopback: terminal text can contain code and secrets. With no token configured it returns `403 control_disabled`; a missing or wrong token returns `401 unauthorized`. HEAD makes the same checks and sends no body. The route is local only; `/api/v1/fleet/peers/{name}/issues/{key}/screen` is `404`.

The engine calls herdr `agent.read` over the socket with `source: visible` and `format: text`, without a line count or CLI fallback. `recent` fails for some working alternate-screen agents. The response carries `key`, `agent_status`, `text`, `lines` (the count of returned text lines) and `sampled_at`; a blocked agent also gets `attach`, a ready-to-copy `herdr agent attach <target>` command. In a named session it begins `herdr --session <name> agent attach <target>`. The target is the worker's herdr agent name.

Reads are cached per worker key for one second; cache hits keep the same `sampled_at`. Every response uses the current configured secrets to redact the screen, then cuts text from the top until the whole JSON body is at most 64 KiB. The end of the last line stays visible, even when that line alone exceeds the cap. Up to eight HTTP herdr calls may overlap across the live-agent and screen routes; a request above the limit gets `503 herdr_busy`.

Herdr `agent_not_found` or `pane_not_found` returns `200` with `state: "not_running"`. Herdr off, scripted, stopped or on a different protocol returns `200` with `available: false` and `reason` (`off`, `scripted`, `not running`, `protocol <n>` or `error: <code>`). Unknown issue keys return `404`.

## Worker background logs

`GET /api/v1/issues/{key}/logs` lists this machine's retained managed-job logs and regular Claude Code task `.output` files for the worker. `GET /api/v1/issues/{key}/logs/{id}` reads one selected log. Both require `Authorization: Bearer <status token>` even on loopback. With no token configured they return `403 control_disabled`; a missing or wrong token returns `401 unauthorized`. HEAD uses the same checks and has no body. Unknown keys and IDs return `404`. IDs are opaque and resolved only from that key's discovered logs; neither route accepts a filesystem path. Fleet peers cannot proxy these routes.

Managed jobs stream both stdout and stderr to a per-lease file as bytes arrive, including output beyond the command's returned-output budget. A log is `running` while its resource lease is active and `done` afterward. Each file keeps its newest 8 MiB on disk; its `size` and offsets count all bytes written. Completed logs are kept for up to 24 hours and at most 20 per worker; active logs are exempt. Claude Code tasks are found under the worker's own worktree slug in the local `claude-<uid>` temporary directory. Only regular `.output` files are listed, so symlinked transcripts are excluded. A task is marked `running` when the worker is running and the file changed in the past three seconds; otherwise it is `done`. A task file's first line becomes its label, or its task ID if that line is empty, unreadable or at least 4 KiB.

The list returns `items` with `id`, `source` (`job` or `claude-task`), redacted `label`, `state`, logical `size`, `started_at` and `updated_at`. The detail returns redacted `text`, effective `offset`, `next_offset`, logical `size`, `state` and `truncated`. Omit `offset` for a tail read or send `?offset=<next_offset>` to follow. If a job's requested offset fell before its retained window, `truncated` is true and the response starts at the first kept byte. An unreadable selected file returns `available: false` and `reason`; an ID absent from the fresh listing returns `404`. Text is read in bounded chunks and the complete JSON response is at most 64 KiB after redaction and escaping.

## Actions

The engine that serves the page can be controlled from it. Peers are never controlled. `POST /api/v1/actions/{op}` sends one control operation to the engine's state loop, the same way `toolu epic` does over its control socket:

| Op | JSON body | Same as |
|---|---|---|
| `pause` | `{}` or `{"epic": KEY}` | `toolu epic pause [EPIC]` |
| `resume` | `{}` or `{"epic": KEY}` | `toolu epic resume [EPIC]` |
| `ack` | `{"key": KEY}`, or `{"kind": KIND}` for a keyless item | `toolu epic ack KEY` |
| `answer` | `{"key": KEY, "text": TEXT}` | `toolu epic answer KEY TEXT` |
| `release` | `{"key": KEY}` | `toolu epic release KEY` |
| `host-pause` | `{"host": HOST}` | `toolu epic hosts pause HOST` |
| `host-resume` | `{"host": HOST}` | `toolu epic hosts resume HOST` |

`answer` types into agents that run with approval bypass, so every action needs all of the following, even on loopback:

- `Authorization: Bearer <status token>`;
- an `Origin` equal to `http://` followed by the request's `Host`;
- `Content-Type: application/json`;
- a `Content-Length` of at most 16 KiB.

The server sends no CORS headers. The checks run in this order:

| Status | Error | When |
|---|---|---|
| `405` | `method_not_allowed` | The method is not `POST`; the response has `Allow: POST`. |
| `403` | `control_disabled` | No status token is configured. |
| `401` | `unauthorized` | The token is missing or wrong. |
| `403` | `foreign_origin` | `Origin` is missing or foreign. |
| `415` | `unsupported_media_type` | The body is not JSON. |
| `404` | `unknown_op` | The op is not in the table above. |
| `400` | `invalid_request` | The body is not an object, or a field is invalid. |

Field rules:

- Keys, epics and kinds match `[A-Za-z0-9._-]{1,128}`.
- `ack` takes exactly one of `key` or `kind`.
- `answer` text is 1 to 4,000 characters, with no control characters other than newline and tab.
- `host-pause` and `host-resume` take a `host`: `claude`, `codex`, `cursor` or `muse` (aliases `claude-code`, `cursor-agent` accepted); anything else is `400 invalid_request`.

A `POST` without `Content-Length` is `400`, and a larger body is `413`.

Replies:

- **`200`:** the engine's reply, which has no `schema` field. An `answer` whose delivery herdr cannot confirm (`timeout` or `agent_prompt_stalled`) replies `{"ok": true, "delivered": "uncertain", "note": …}` and leaves the attention item open.
- **`409`:** an engine refusal such as `no live agent for <key>` or `<key> is not held`. The body is `{"schema": 1, "error": <text>}`.
- **`503 engine_busy`:** the state loop did not answer within 35 s, longer than herdr's 30 s prompt timeout. The server never retries, and the action may still apply later.

Web actions are journaled like CLI actions, with `by=web` in the note. `pause` and `resume` write `action pause` or `action resume` with the note `<epic|all> by=<who>`. `host-pause` and `host-resume` write `action host-paused` or `action host-resumed`, keyed by the host, with the note `by=<who>`. So every open dashboard sees the change on `/api/v1/events/stream`.

## Host pause

A host pause lives in the shared resource store (`paused_hosts`, next to `cooldowns`), so it is machine-wide and survives an engine restart. Routing skips a paused host like a cooling one, but `/api/v1/hosts` reports the two apart: each item carries `paused`, `paused_by` and `paused_at` beside `cooldown_until` and `reason`, and a host can be paused and cooling at once. Pausing never stops a running worker; a relaunch of a running worker whose host is paused waits with the reason `host paused` and keeps its session for the resume. Resuming clears the pause and any active cooldown — in the shared store and in every registered epic's `hosts.json` — so the next route may pick the host again; if the provider is still limited, the next limit line cools it again. A manual `toolu epic launch --kind <paused host>` is refused with `host <name> is paused; resume it or pass --force`; `--force` launches anyway.

`toolu epic token url` prints `http://<address>/#token=<token>` for the running engine. The engine writes its bound address to `http.json` under its resource root while it runs. A `0.0.0.0` or `::` bind prints `127.0.0.1`. The token stays in the URL fragment: it never goes on a command line, in a query string or in a log line.

The listener re-reads `toolu/secrets.json` at most once a second when its content changes, so `toolu epic token new` takes effect without a restart. If a read fails, the last good token stays in use. A token from `TOOLU_EPIC_STATUS_TOKEN` or `TOOLU_EPIC_TOKEN` can change only with a restart.

## Fleet over a private network

An engine can show other machines' engines beside its own. List each peer under `epic.peers` in `toolu.config.json` with a `name` and a `url`, and put its bearer token in `peer_tokens` of `toolu/secrets.json` (mode 0600) or in `TOOLU_EPIC_PEER_TOKENS`, a JSON object of peer name to token. A token never goes on the command line. The engine reads its peers at startup; `toolu epic status --fleet` reads them on every run.

A peer accepts only a `Host` equal to its bound address, so the URL names the peer's bound IP and port, for example `http://192.168.1.20:7717` for an engine started with `--http 0.0.0.0:7717` or `epic.http.bind` `0.0.0.0`. A DNS name, or a URL without the port, gets `403` (state `unauthorized`, reason `host`). With TLS in front of a peer, the proxy must forward to the engine with `Host` set to the engine's address: nginx's `proxy_pass` does this by default, and Caddy needs `header_up Host {upstream_hostport}`.

A token sent to an `http://` peer crosses the network in clear text. Use plain HTTP only on a private network. A status server must not be exposed to the public internet without its token and TLS in front of it.

`/api/v1/fleet` lists this machine first (`name` `local`, `local: true`), then each peer in config order. Peers are fetched in parallel with a 2 s deadline, at most once a second for requests and every 30 s in the background while peers are configured. A peer's `state` is `ok`, `unreachable` (no reply, timeout, redirect, a body over 1 MiB or a server error), `unauthorized` (`reason` `token` for `401`, `host` for the engine's `403 foreign_host`, otherwise `null`) or `version-mismatch` (a status API other than schema 1). `last_seen` is the peer's last `ok` reply since this engine started, and `age_ms` its age; only `ok` machines carry data and count in `summary`. In-flight issues are those whose stage is `running` or `awaiting_merge`. Peer fields are allow-listed, strings are cut to 160 characters, and every configured secret is redacted. Peer requests are read-only `GET`s; a peer is never asked to act.

`/api/v1/fleet/peers/{name}/{route}` returns a peer's own document, redacted, so the browser needs only this server's address. `{name}` is percent-decoded and matches a configured peer exactly; `{id}` and `{key}` match `[A-Za-z0-9._-]{1,128}`. An unknown peer or route is `404`; a failing peer is `502` (`peer_unreachable`, `peer_unauthorized`, `peer_version_mismatch`), and more than eight proxied calls at once get `503 proxy_limit`.

## Dashboard

The dashboard at `/` serves embedded HTML. `/assets/<binary-version>/` serves embedded CSS, JS and Latin-subset variable fonts; these routes have immutable caching, while the root HTML and JSON do not. The font OFL files remain in the repository and are not served. The strict Content-Security-Policy allows same-origin scripts, styles, fonts and connections, and forbids inline script and style.

The page stacks every registered epic and shows four tiles, hosts, attention and seven recent journal events. Local documents update from `/api/v1/events/stream`; `/api/v1/agents/live` supplies terminal titles every 3 seconds only while the tab is visible. A peer is read-only: its overview, epic, host and attention documents use the GET-only fleet proxy every 5 seconds while visible, and the browser never requests peer live-agent or event routes. Waiting jobs are recent refused admissions; their markers expire after two minutes. Queue entries are the engine's merge queue: per repository, ready issues with a pull request, ordered by `unblocks`, then ready time, then key; `head` marks the one the engine works.

Selecting a worker row, or an attention item's key, opens that worker's drawer beside the board; Esc closes it and returns focus to the row. The drawer shows the worker's header, agent status and live title, open attention, its plan ledger steps (`✓ GREEN`, `● IN PROGRESS`, `○ TODO`, `✕ FAILING`, `↺ STALE`), its visible screen and its phase history. While the drawer is open and the tab is visible, it reads `/api/v1/issues/{key}` and, for a launched local worker, `/api/v1/issues/{key}/screen` every 2 seconds; a closed drawer or a hidden tab makes no such request. A blocked agent shows the screen's `attach` command with a copy button. With a peer selected, the drawer reads the peer's issue document through the fleet proxy and never requests a screen or live status.

The URL fragment is an `&`-separated list. `#issue=<key>` opens that worker's drawer and stays in the address bar while it is open. `#token=<token>`, from `toolu epic token url`, is read on load or when the fragment changes, and kept in the tab's `sessionStorage`; the page then removes only that key from the address bar with `history.replaceState`. Screen reads send the token as `Authorization: Bearer`, never in a URL, and a `401` drops the stored token.

The top bar shows `○ CONTROL LOCKED [ UNLOCK ]` or `● CONTROL UNLOCKED [ LOCK ]`. `[ LOCK ]` forgets the token; `[ UNLOCK ]` only explains how to get the token URL, since the page has no token input. Unlocked, the page offers `PAUSE ALL EPICS` and `RESUME ALL EPICS`, `PAUSE EPIC` or `RESUME EPIC` on each epic, `PAUSE` or `RESUME` on each host (`RESUME` also on a cooling host, and it clears both), and one action per attention item: `[ ANSWER ]` for `needs-human`, `stuck`, `help` and `contract-changed`, `[ RELEASE ]` for `coupling-hold`, and `[ ACK ]` for every other kind. The drawer offers `ANSWER →` while the worker's agent is live, `RELEASE HOLD` when the issue has a `hold`, and `ACK` when it has open attention. `ANSWER` opens a dialog with the worker's question and a 4,000-character answer; it reports `✓ DELIVERED`, `● REFUSED · <code>` (the engine's 409, with herdr's code such as `agent_blocked` when there is one) or `▲ UNCERTAIN`. Every action is one `POST /api/v1/actions/{op}` with `Authorization: Bearer` and `Content-Type: application/json`; buttons are disabled while locked or while a request is pending. Other actions show `✓ SENT <journal line>`, and the change appears when the journal stream refreshes the page. Any `401` forgets the token and shows `● 401 · TOKEN ROTATED`. A peer view renders no action control and never sends a `POST`.
