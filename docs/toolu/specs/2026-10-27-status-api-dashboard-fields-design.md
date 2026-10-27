# Status API fields for the redesigned dashboard — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** Falconiere/toolu#657, part of #649

## Problem

The redesigned dashboard (#658) shows a board header (`WAVE 1 OF 4 · 3 LIVE · 4 QUEUED`), the epic title and link, each worker's model, effort, phase age and coupling hold, the machine name (`LOCAL · STUDIO-MBP`), and a link to each fleet peer's own dashboard. The status API serves none of these, so the page would have to guess or read files.

## Non-Goals

1. No `agent_status` / `agent_status_at_ms`: #651 adds them to the same rows.
2. No change to `control.rs` `epic_key` (it still reads `graph.epic` as a string).
3. No new routes, no page code, no change to the peer proxy's route list or redaction.
4. No relaxing of `epic.peers[].url` validation.
5. `hold.score` and `hold.source` are never exposed.

## Architecture

All new fields are computed where the existing documents are built, so the HTTP API, the control socket and `toolu epic status --json` (which calls `status_api::document`) return the same values, and every document still passes through `secrets::redact_json`.

- **Epic header** (`status_epics.rs`): one `header(world, key)` builds `title`, `url`, `waves_total`, `current_wave`, `live`, `queued` from the engine's own parsed graph (`world.epics[key].graph`, a `graph::Graph` whose `epic` is the `EpicInfo` of `graph.json`) and the issues' live stages in `World`. It is merged into both `summary` (list) and `detail`. Using the parsed graph avoids a second `graph.json` read per epic on every list request; the engine reloads it on every refresh.
  - An issue is **done** when its `World` stage is `merged` or `abandoned`, or its graph `status` is `done` (closed in the tracker before the engine saw it).
  - `live`: `World` issues of the epic whose stage is `running` or `awaiting_merge` (same rule as `fleet.rs` `in_flight`).
  - `queued`: `World` issues of the epic with an empty stage (never launched; `snapshot::load_issue` inserts every graph issue with stage `""`) that are not done.
  - `waves_total`: distinct `wave` values of the graph issues.
  - `current_wave`: the lowest wave holding an issue that is not done, else `null`.
  - `title` and `url`: `EpicInfo.title` / `EpicInfo.url`, cut to 160 characters, `null` when empty or when the epic has no parsed graph.
- **Issue rows** (`status_epics::issue_row`): add `phase_age_ms` (`now_ms - phase_at_ms`, as `/api/v1/issues/{key}`), `model` and `effort` through one helper shared with `status_api::issue_document` (route record first, then the launch record), and `hold`.
- **`hold`** on rows and on `/api/v1/issues/{key}`: `{"kind", "with", "at"}` taken from `issue.hold` (each a string cut to 160 characters, or `null`), or `null` when there is no hold.
- **`hostname`**: new `toolu_runtime::machine::short_hostname()` using `nix::unistd::gethostname` (enable the `hostname` feature beside `user` on the runtime crate's `nix`), cut at the first `.`; `None` when the call fails or the name is empty or not UTF-8. `status_api::document` (overview) inserts it; `fleet::project` copies an `ok` machine's overview `hostname` (so the local entry has it, and a peer running this version has its own).
- **`dashboard_url`**: `fleet::Machine` gains `dashboard_url: Option<String>`, `None` for the local machine and `Some(peer.url)` for each peer. `config::epic::peer_url` already refuses any peer URL with a user name, password, path, query or fragment and stores `Url::to_string()` of the accepted origin, so the configured URL is already the credential-free dashboard URL. Jev (choice, 0.97) preferred this to a second stripping function that could never see such a URL.

## Interfaces / Schema

```jsonc
// /api/v1/epics items[] and /api/v1/epics/{id} (and the overview's epics[])
{"key": "e", "paused": false, "counts": {...}, "attention_count": 0,
 "title": "Epic: realtime dashboard", "url": "https://github.com/o/r/issues/1",
 "waves_total": 4, "current_wave": 1, "live": 3, "queued": 4}
// /api/v1/epics/{id} issues[]
{"key": "o-r-2", "stage": "running", "phase": "execution", "pr": 102, "host": "claude",
 "plan_summary": null, "model": "opus", "effort": "high", "phase_age_ms": 1200,
 "hold": {"kind": "coupling", "with": "o-r-3", "at": "2026-10-27T12:00:00Z"}}
// /api/v1/issues/{key}: adds "hold" (same shape or null)
// /api/v1/overview: adds "hostname": "studio-mbp"
// /api/v1/fleet machines[]: adds "hostname" (string or null) and "dashboard_url"
//   (null for local, the peer's configured origin for a peer)
```

```rust
// crates/core/runtime/src/machine.rs
pub fn short_hostname() -> Option<String>;
```

## Failure modes and edge cases

- Epic registered but no parsed graph (unreadable or not yet loaded): `title`, `url`, `current_wave` are `null`, `waves_total` 0; `live`/`queued` still count `World` issues. The rest of the document is unchanged.
- Graph issues without a `wave`: not counted in `waves_total` and never pin `current_wave`.
- Every issue merged or abandoned: `current_wave` is `null`.
- An `abandoned` issue in wave 1 with the rest merged: wave 1 is done.
- `hold` not an object, or missing fields: fields are `null`; `score`/`source` never appear.
- Missing route or launch record for a row: `model`/`effort` are `null`; the row and board stay intact (as `plan_summary` today).
- `gethostname` failure: `hostname` is `null`; the overview never fails for it.
- A peer that is `unreachable`: `hostname` is `null`, `dashboard_url` is still set (the operator can still open it).
- Title over 160 characters: cut to 160 characters. A configured secret inside a title or URL: replaced by `redact_json` like every field.
- A peer URL with credentials, query or fragment: config load fails as today with `expected a credential-free HTTP(S) origin`; the secret never appears in the error or any response.

## Acceptance criteria

- **AC-1:** For a real epic whose `graph.json` is written by the engine's refresh (the code behind `toolu epic graph --save`) from the GitHub fixture, `/api/v1/epics/{id}` and the list return that `title` and `url`, `waves_total` equal to the graph's distinct waves, and the `current_wave`, `live` and `queued` the stages imply (e.g. two running wave-1 issues and two unlaunched later issues give `current_wave` 1, `live` 2, `queued` 2).
- **AC-2:** After the wave-1 issues merge, `current_wave` is 2; after every issue merges it is `null`. An `abandoned` issue never pins a wave.
- **AC-3:** Each issue row's `model`, `effort` and `phase_age_ms` equal those of `/api/v1/issues/{key}` for the same worker; a held issue's row and issue document carry `hold` `{kind, with, at}` without `score` or `source`.
- **AC-4:** `/api/v1/overview` `hostname` and the local `/api/v1/fleet` entry's `hostname` equal the test machine's `hostname` output cut at the first `.`.
- **AC-5:** A peer configured as `http://10.0.0.2:7717/` gets `dashboard_url` `http://10.0.0.2:7717/`; a peer configured as `http://user:pass@10.0.0.2:7717/?x=1#y` is refused at config load, and `user:pass` appears in no error or response.
- **AC-6:** An epic title over 160 characters is served cut to 160 characters, and a configured secret inside the title is redacted.
- **AC-7:** `cargo xtask gate` passes.
- **AC-8:** On a real engine, the control socket's `status` op for the epic (what `toolu epic status --json <epic>` prints) carries the same `title`, `url`, `waves_total`, `current_wave`, `live`, `queued` and row fields as `GET /api/v1/epics/{id}`.
- **AC-9:** Through the peer proxy, `/api/v1/fleet/peers/{name}/epics/{id}` of a real peer engine carries the peer's header and row fields, and a configured secret in a proxied title is redacted.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `scenario_rig::Scenario` (GitHub HTTPS fixture, real checkout, herdr fixture) building `graph.json` by refresh; real `Engine` pumping launches | header fields as stated, list and detail agree | epic with no graph (unit) | `cargo test -p toolu-epic-orchestrator --lib status_board::` (new `tests/status_board_test.rs`) |
| AC-2 | same scenario; `close` + `report ready` merges wave 1, then the rest | `current_wave` 1 → 2 → `null` | abandoned issue (unit over `World`) | same filter |
| AC-3 | scenario engine; plus a unit with route/launch records and a `hold` | row fields equal the issue document's | no route record gives `null` | same filter + `status_epics::` |
| AC-4 | the real machine; `/bin/hostname` output as the independent oracle | `short_hostname()`, overview `hostname` and the local fleet entry all equal it cut at `.` | name with a `.` is cut (unit on the cutting function) | `cargo test -p toolu-runtime machine::` and `http_fleet::` two real engines |
| AC-5 | two real engines (`fleet_rig`) and `config::epic::parse` | `dashboard_url` is the peer origin; dirty URL refused, no `user:pass` | local entry `null` | `http_fleet::`, `config::tests::epic` |
| AC-6 | a `graph.json` written by `graph::write_graph` whose `epic.title` is 200 characters and holds a secret loaded by `secrets::load` from `TOOLU_EPIC_PEER_TOKENS`; the epic loaded into `World` by `snapshot` and served by `status_api::route` | title has 160 characters and `<redacted>` in place of the secret, on the list and the detail | title exactly 160 characters is unchanged | `status_epics::` |
| AC-7 | repository | gate passes | — | `cargo xtask deploy-gate` (full `cargo xtask gate` in the container) |
| AC-8 | `fleet_rig::Engine` (real engine, socket and listener) | socket `status` epic document equals the HTTP one on the new fields | — | `http_fleet::` |
| AC-9 | a real peer engine (`fleet_rig::Engine`) whose `graph.json` `epic.title` holds a secret, proxied by a real listener whose secrets carry it in `TOOLU_EPIC_PEER_TOKENS` | `waves_total`, `live`, row `phase_age_ms` present; title `<redacted>` | — | `http_fleet::proxy_tests::` |

## Documentation impact

`docs/epic-status-api.md` (generated from `status_schema.rs`): the new fields in the epics, epic, issue, overview and fleet rows, and a short "Board header" paragraph defining done, `live`, `queued` and `current_wave`, `hold`, `hostname` and `dashboard_url`.

## Open Questions

None blocking.
- AC5's literal URL cannot reach the fleet code because config refuses it; decided (Jev 0.97) to prove AC5 through that refusal plus the clean origin. Owner: this worker.
- `hostname` is also copied for peers (from their overview) since the projection is per machine; the issue requires only the local entry. Non-blocking.

## Spec review (2026-10-27)

- Acceptance criteria: 🟡 should-fix (fixed): issue scope "`toolu epic status --json` returns the same fields" and "the peer proxy forwards the new fields" had no AC. Added AC-8 and AC-9.
- Acceptance evidence: 🟡 should-fix (fixed): AC-6 and AC-9 named a scripted document; both now use a real `graph.json` and real engines. AC-4 names `/bin/hostname` as the independent oracle.
- Acceptance evidence: 🔵 consider: the `abandoned` boundary of AC-2 runs over a `World` built in the test, not the scenario; the scenario has no abandon path cheaper than its relaunch limit. Kept.
- Jev (noul): scope coverage 0.61 → 0.77 after the fixes, AC5 treatment consistent 0.84, evidence completeness 0.62 (uncertain; the remaining items were checked by hand: every AC row names input, expected result and a runnable filter).
