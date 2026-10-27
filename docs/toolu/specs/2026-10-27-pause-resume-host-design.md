# Pause and resume an agent host — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Falconiere Barbosa (epic #649 worker)   **Topic:** Pause/resume a worker host from the CLI and the dashboard (#653)

## Problem

An operator cannot take one agent host (Claude Code, Codex, Cursor Agent, Muse) out of rotation without pausing whole epics or waiting out a usage-limit cooldown. Saving one provider's quota, or clearing a cooldown the operator knows has reset, needs a host-level switch on the CLI and on the dashboard (#656 builds the UI on these fields and routes).

## Non-Goals

1. No dashboard UI (#656), no worker drawer changes (#655).
2. No control of fleet peers; the peer proxy stays read-only.
3. No stopping, killing or relaunching workers that are already running on a paused host.
4. No rerouting a paused host's waiting relaunch to another host (it would lose the worker's session).
5. No editing host caps or config.

## Architecture

**Storage (`host_pause`, new module in `toolu-epic-orchestrator`).** The pause lives in the shared resource store (`<resource root>/state.json`), machine-wide like the cooldowns `limit.rs` writes there, under a new key next to `cooldowns`:

```json
"paused_hosts": {"codex": {"by": "user", "at": 1760000000000}}
```

`by` is `user` (CLI/socket) or `web` (HTTP action); `at` is epoch milliseconds. Writes go through `toolu_engine::resources::store::update_resources`, so they are locked and atomic with cooldown writes. `read_resource_state` accepts the key as-is (verified on the issue: `is_resource_state` checks only `version`, `leases`, `cooldowns`, `pressure`). Pause survives an engine restart because routing reads the store, not engine memory. Pausing an already-paused host refreshes `by`/`at` and journals again; resuming a host that is not paused still clears its cooldowns and journals.

**Control socket (`dispatch.rs`).** Two new ops, `host-pause` and `host-resume`, each `{host, by?}` (`by` defaults to `user`; the HTTP layer sends `web`). The handler validates the host with `hosts::host_kind` (aliases `claude-code`/`cursor-agent` accepted; the stored key is the canonical name), applies the store change on the state thread (the only writer), journals, and replies `{"ok": true}`. An unknown host replies `{"error": "unknown host <name>; use one of claude, codex, cursor, muse"}`. Journal records (via the outbox, like `journal_pause`):

- `action host-paused`, key = host name, note `by=<who>`;
- `action host-resumed`, key = host name, note `by=<who>`.

So a CLI pause and a web pause produce the same record except `by` (AC4), and every dashboard sees it on `/api/v1/events/stream`. After a `host-resumed`, the handler pumps (`pump_after`) so waiting launches re-arm.

**Resume clears both cooldown stores.** `limit.rs` writes a cooldown twice: the shared store (`write_cooldown`) and the epic's `state_dir/hosts.json` (`write_host`); `Capacity::load` merges both. Resume therefore removes the host from (a) `paused_hosts` and `cooldowns` in the shared store, and (b) the host key in `hosts.json` of every registered epic (the engine's registry `registry.json` `epics[].state_dir`, plus any epic state dir in `World` not in the registry). A missing `hosts.json` is skipped; a file that becomes empty is written back as `{}`. If the provider is still limited, `limit.rs` cools the host again on the next limit line — resume does not block that.

**Routing (`route/capacity.rs`, `route/assign.rs`).** `Capacity` gains `paused: BTreeSet<HostKind>`, filled in `Capacity::shared` from the store's `paused_hosts` keys (canonical names only). `pick_host` is called with `cooling ∪ paused`; `Pass::keep` also refuses a saved host that is paused. Cooling and paused stay separate fields so status can tell them apart. New issues therefore route to other hosts with capacity (AC1); a route with no open host keeps `host: null` as today.

**Waiting relaunch (`effects_launch.rs`).** Before routing, a non-reroute launch whose launch record is a running worker (`stage` `running`, record `kind` naming the host) on a paused host returns `{"outcome": "wait", "note": "host paused"}` without touching the saved route. The existing wait path (`note_wait` + `mark_relaunch` in `commit.rs`) retries it after `retry_at_ms`; once resumed, routing keeps its saved host and the relaunch proceeds with its session. A reroute (armed by `limit.rs` after a limit) is exempt: it routes away as today.

**Manual launch (`launch.rs`).** After `resolve_host`, a launch whose resolved host is paused is refused with `host <name> is paused; resume it or pass --force`, unless `--force` is set (the engine's own launches set `force`, and are handled by the waiting rule above, never by this refusal). The check reads the shared store under the launch's resource home; an unreadable store fails the launch as today (`Failure::unknown` from the admission path conventions).

**CLI (`verbs_hosts.rs`, wired in `verbs.rs`).**

```
toolu epic hosts pause <host>
toolu epic hosts resume <host>
```

`<host>` is a positional argument (never `--host`, a global flag elsewhere). The verb validates the name with `host_kind` first (unknown host: `toolu epic hosts pause: unknown host <name>; use one of claude, codex, cursor, muse`, exit `Failure`), then sends `{"op": "host-pause"|"host-resume", "host": <canonical>, "by": "user"}` through the control socket with `ask_checked`, so an engine refusal is also a CLI failure. It prints the engine reply (`{"ok":true}`) like the other action verbs.

**HTTP actions (`http_action.rs`).** The allow-list gains `host-pause` and `host-resume`. Their body is `{"host": "<host>"}`; the host must parse with `host_kind`, else `400 invalid_request` (AC5). They forward `{"op": <op>, "host": <canonical>, "by": "web"}` under the unchanged #650 gate (token, same-origin `Origin`, JSON, 35 s deadline, `409` on engine refusal, `503 engine_busy`, no CORS).

**Status (`status_resources.rs`, `status_schema.rs`).** Each `/api/v1/hosts` item gains:

- `paused`: whether the host is in `paused_hosts`;
- `paused_by`: the stored `by`, or `null`;
- `paused_at`: ISO-8601 milliseconds of `at`, or `null`.

`cooldown_until`/`reason` are unchanged, so a host can be paused and cooling at once (the state #656 renders as `PAUSED · COOLING`). The overview document carries `hosts` through the same snapshot, so it gains the fields with no extra work, and the peer-proxy `hosts` document forwards them through its existing redaction. The fleet machine rows keep today's allow-listed fields (`host`, `running`, `cap` in `fleet::host_row`); fleet-wide host state is out of scope here.

## Interfaces / Schema

```
Socket:  {"op": "host-pause",  "host": "codex", "by": "user"|"web"}  -> {"ok": true} | {"error": "unknown host …"}
         {"op": "host-resume", "host": "codex", "by": "user"|"web"}  -> {"ok": true} | {"error": …}
HTTP:    POST /api/v1/actions/host-pause   {"host": "codex"}   (Bearer [REDACTED] + Origin + JSON, as #650)
         POST /api/v1/actions/host-resume  {"host": "codex"}
CLI:     toolu epic hosts pause|resume <host>
Store:   state.json "paused_hosts": {"<host>": {"by": "user"|"web", "at": <epoch ms>}}
Status:  /api/v1/hosts items gain paused (bool), paused_by (string|null), paused_at (ISO ms|null)
Journal: action host-paused / host-resumed, key=<host>, note "by=<who>"
```

## Failure modes and edge cases

- **Unknown host:** CLI/socket list the valid hosts (`unknown host <name>; use one of claude, codex, cursor, muse`); HTTP validation answers `400 invalid_request` before forwarding. Aliases canonicalize, so `cursor-agent` pauses `cursor`.
- **No engine running:** the CLI verbs go through `ensure`/`ask_checked` like `pause`/`resume`; a refused engine reply is a CLI failure, never a silent success.
- **Store unreadable or invalid:** `read_resource_state` errors propagate (routing fails the routing pass, status returns its existing error, the socket op replies `{"error": …}` → HTTP `409`). No partial write: `update_resources` writes once under the lock.
- **Resume with no pause and no cooldown:** still `{"ok": true}`, journaled; both stores simply lose nothing.
- **Paused and cooling at once:** status shows both; routing skips the host for either reason; resume clears both.
- **All hosts paused:** fresh routes get `host: null` and launches wait with `no host capacity`, as when every host cools.
- **Running workers:** untouched — no lease, pane or record change on pause.
- **Concurrent CLI + web pause:** both serialize on the state thread and the store lock; last write wins for `by`/`at`.

## Acceptance criteria

- **AC-1:** With `codex` paused in the shared store, a routing pass over a scripted epic routes no new issue to Codex while Claude has capacity; a Codex worker already running keeps its lease and record unchanged.
- **AC-2:** With an active cooldown written through `limit.rs`'s real path (a limit line fed to a worker) plus a pause, resuming `codex` makes `/api/v1/hosts` show it neither paused nor cooling, and `Capacity::load` for each registered epic reports it neither cooling nor paused; the next route may pick it.
- **AC-3:** A pause written to the store is still in effect after the engine restarts: a fresh `Capacity::load` / routing pass against the same resource home still skips the host.
- **AC-4:** A pause sent over the control socket (`by` absent → `user`) and a pause sent over HTTP produce the same journal record (`action host-paused`, key, note shape) except `by=user` vs `by=web`.
- **AC-5:** An unknown host fails on the CLI with the valid-host list (failure exit), on the socket with the same message, and over HTTP with `400`.
- **AC-6:** `cargo xtask gate`, `cargo xtask check-cli-compat` and `cargo xtask check-markdown-cli` pass; the CLI contract snapshot (`docs/cli/commands.json`, `fixtures/commands_json.snap`) is regenerated.
- **AC-7:** A running worker whose agent is lost while its host is paused waits with the reason `host paused` (journaled `launch-wait`), keeps its saved route and session, and relaunches on the same host after resume — it is never routed to another host.
- **AC-8:** `toolu epic launch --kind <paused host>` for a ready issue is refused with `host <name> is paused; resume it or pass --force`; the same launch with `--force` proceeds past that refusal.

## Acceptance evidence

| AC | Real input | Expected | Check |
| --- | --- | --- | --- |
| AC-1 | Real engine + scripted epic under a temp `TOOLU_RESOURCE_HOME`; `host-pause codex` over the socket, then a route/launch batch; a running Codex worker record + lease | No new route names `codex` while `claude` has room; running worker untouched | `cargo test -p toolu-epic-orchestrator host_pause` (route/capacity + effects tests) |
| AC-2 | Cooldown written by feeding a limit line (`limit.rs` path) in a real engine; pause, then `host-resume`; GET `/api/v1/hosts` | `paused:false`, `paused_by:null`, `paused_at:null`, `cooldown_until:null`; `Capacity::load` per epic clean | same, plus `status_resources` test |
| AC-3 | Store written by one engine/`Capacity` read, then a fresh engine and routing pass on the same home | Host still skipped | same |
| AC-4 | Real engine with HTTP listener (the `http_action` rig): socket pause vs `POST /api/v1/actions/host-pause` | Identical journal rows except `by` | `cargo test -p toolu-epic-orchestrator http_action` |
| AC-5 | `toolu epic hosts pause nope` on the built binary; socket op and HTTP POST with `"host":"nope"` | CLI failure listing hosts; socket error; HTTP 400 | binary/CLI test + `http_action` test |
| AC-6 | Whole workspace | Green | `cargo xtask gate`, `check-cli-compat`, `check-markdown-cli` |
| AC-7 | Real engine, running Codex worker record (`stage: running`, saved route `codex`), host paused, agent lost → relaunch armed; then resume and retry due | Wait outcome with note `host paused`; saved route still `codex`; after resume the relaunch proceeds on `codex` | `cargo test -p toolu-epic-orchestrator host_pause` (effects_launch/commit path) |
| AC-8 | Built `toolu` binary (or `launch_issue` with a live Ctx seam) against a temp resource home with `codex` paused, ready issue, `--kind codex`, then `--force` | Refusal message exactly; with `--force` the refusal is gone (launch proceeds to its next stage or dry-run output) | `cargo test -p toolu-epic-orchestrator launch` / CLI test |

Test plan per the issue: a real engine with a scripted epic, real routing, and the shared resource store under a temporary `TOOLU_RESOURCE_HOME`; the cooldown is written by feeding a limit line through `limit.rs`'s real path. No mocks.

## Documentation impact

- `docs/cli/`: regenerated with `cargo xtask docs-cli` (new `hosts` verbs), plus the CLI contract snapshot.
- `docs/epic-status-api.md`: regenerated from `status_schema.rs` (`/api/v1/hosts` fields, two new action rows, journal note).
- `docs/epic-orchestrator/README.md`: the hosts section gains pause/resume (CLI, dashboard actions, machine-wide store, resume clears cooldowns).

## Open Questions

None blocking. The three decisions the issue left to the spec — storage (shared store), relaunch behavior (wait, `host paused`, no reroute), and CLI names (`toolu epic hosts pause|resume <host>`) — are settled above from the issue's review additions and a Jev comparison recorded in the brainstorm (`wait` 0.97, two socket ops 1.00, routing+manual enforcement 0.92).

## Spec review (2026-10-27)

- Status (fleet claim): 🟡 should-fix: the fleet machine rows allow-list `host`/`running`/`cap` in `fleet::host_row`, so they do not gain the new fields automatically. Fixed: the Architecture section now scopes fleet rows out explicitly.
- Acceptance criteria: 🟡 should-fix: the waiting relaunch (AC-7) and the manual-launch refusal/`--force` (AC-8) had sections but no AC or evidence row. Fixed: AC-7 and AC-8 added with real-input checks.
- Jev requirement-coverage score was 1.38/2 (partial↔full, confidence 0.43) before these fixes, pointing at exactly those two gaps; after them every issue requirement and review addition maps to a section and an AC by manual cross-check.
