# Dashboard worker drawer — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Claude Code   **Topic:** drawer with a worker's plan, live status and screen (#655)

## Problem

Dashboard rows are not clickable. To see a worker's plan steps or its terminal, an operator reads `toolu ledger status` in the worktree or attaches to the herdr pane. #651, #652 and #654 now serve agent status, the plan ledger and the visible screen; the page does not show them.

## Non-Goals

1. Actions. `ANSWER →`, `RELEASE HOLD` and `ACK` get an empty reserved slot; #656 wires them.
2. The lock chip, locked banner, `[ LOCK ]` and 401 banner (#656). This change only reads the token that #656's chip will manage.
3. Key presses from the web, peer screens, peer live status.
4. Job rows. The page renders no job rows today, so nothing opens from them.
5. Engine or API changes. Every field the drawer needs is already served.

## Architecture

New embedded files, each under the 300-line limit and wired into `src/dashboard.rs` and the asset route test:

- `js/logic/drawer.js` — DOM-free: `stepMark(status)`, `planLine(plan)`, `nextStep(plan)`, `screenNotice(context)`, `attachCommand(screen)`, `phaseTimeline(history, issue)`, `prUrl(epicUrl, pr)`.
- `js/logic/fragment.js` — DOM-free: `parseFragment(hash)` and `formatFragment(map)` for the `&`-separated fragment.
- `js/view/drawer.js` — renders the drawer from one model; owns the 2 s timer and copy button.
- `drawer.css` — drawer layout and the open-row highlight (`--row-hover`, `#1B1D18`).
- `js/api.js` gains `readIssue(peer, key)` (local `/api/v1/issues/{key}`, peer `/api/v1/fleet/peers/{name}/issues/{key}`) and `readScreen(key, token)` with `Authorization: Bearer`. `fetch` stays only in `api.js`.

`index.html` gets an `<aside id="drawer" class="drawer" hidden>` beside the board inside `main`, with the regions `drawer-header`, `drawer-status`, `drawer-attention`, `drawer-plan`, `drawer-screen`, `drawer-phases`, `drawer-actions`. The layout becomes a two-column grid while the drawer is open (stacked under 900 px).

Data per open drawer: the board row (title, epic number, paused, `open_blockers`, stage) already in `state.docs`, the issue document (host, model, effort, phase, PR, `agent_status`, `plan`, `phase_history`), the attention item for the key, the live item for the title (local only) and the screen (local only, token present). The view keeps one `setInterval(…, 2000)` that exists only while the drawer is open; each tick returns early when `document.visibilityState !== "visible"`. Closing clears the timer. A tick fetches the issue document and, when local, not done and a token is present, the screen. Journal-driven refreshes also re-render the drawer from the latest board docs.

Token: on load `app.js` parses the fragment. A `token` key goes to `sessionStorage["toolu.token"]`; the fragment is rewritten with `history.replaceState` to the remaining keys (`#issue=<key>` or none). `#issue=<key>` alone stays in the address bar. Opening sets `#issue=<key>` and closing removes it, both with `replaceState`; a `hashchange` opens the named drawer. A `401` from the screen route removes the stored token (the same rule #656 applies) and shows `○ CONTROL LOCKED`.

Opening: each board row button and each attention item's key button opens the drawer (click or Enter, since they are buttons). The opener element is remembered; Esc anywhere on the page while open closes the drawer and focuses the opener, or the row with the same `data-issue` when the opener was re-rendered. The open row gets `aria-current="true"` and the highlight.

## Interfaces / Schema

Step mapping (`stepMark`): `green` → `✓ GREEN` muted; `running` → `● IN PROGRESS` accent, blinking; `pending` → `○ TODO`; `red` → `✕ FAILING` error; `stale` → `↺ STALE` muted; any other → `? UNKNOWN` muted. Returns `{kind, label}`; kinds are `green`, `running`, `pending`, `red`, `stale`, `unknown`.

`planLine(plan)` → `"<green>/<total> GREEN"`; `nextStep(plan)` → the title of the step whose `id` equals `plan.next`, else `plan.next`, else `null`.

`screenNotice({peer, issue, token, screen, error})` returns `null` (show the screen) or `{kind, label, text}`, first match wins:

| Condition | Label | Text |
| --- | --- | --- |
| `peer` | `◇ OPEN DASHBOARD TO WATCH OR CONTROL` | Screens, live status and actions stay on the peer. This view shows the plan only. Plus `[ OPEN <NAME> DASHBOARD ↗ ]` linking the peer's `dashboard_url` when `peerDashboardUrl` accepts it. |
| no `issue.stage` | `/ AGENT NOT RUNNING` | Queued until #<n> merges. The screen appears once a host claims it. (`<n>` = open blockers joined `, #`; with none: "Queued. The screen appears once a host claims it.") |
| done stage (`merged`, `cleaning`, `cleanup-incomplete`, `abandoned`) | `/ AGENT NOT RUNNING` | Finished · last phase <PHASE>. |
| no token, or screen HTTP 401/403 | `○ CONTROL LOCKED` | Screen reads need the token. Open the token URL the engine printed. |
| `screen.state === "not_running"` | `/ AGENT NOT RUNNING` | The agent pane is closed. Plans and phases still update. |
| `screen.available === false` or a request error | `? LIVE VIEW UNAVAILABLE` | herdr not reachable. Plans and phases still update. |

`● PLAN UNREADABLE` ("The ledger failed to parse. Phase history shown instead.") replaces the step list when `plan.error` is set; `plan === null` shows "No plan yet. Phase history below until the worker writes its ledger." Decision: the plan notice sits in the PLAN region, not the SCREEN region, because a corrupt ledger says nothing about the screen.

Screen meta: `POLLED 2s · hh:mm:ss` from `screen.sampled_at`, or `LOCAL ONLY` on a peer. Screen body: `<pre class="screen" tabindex="0">` with horizontal scroll.

Blocked block, when `screen.agent_status === "blocked"` and `screen.attach` is a string: `▲ BLOCKED · WAITING ON A KEY PRESS`, the help line, `<code>$ <attach></code>` exactly as served (may begin `herdr --session <name>`), and `[ COPY ]` → `navigator.clipboard.writeText(attach)` → `COPIED ✓` for 2 s; a rejected write shows `SELECT AND COPY`. `attachCommand(screen)` returns the string or `null`.

Header: `<key> · EPIC #<n>`, `‖ PAUSED` when the board is paused, the title; `HOST`, `MODEL`, `EFFORT` (`—` when null); `phaseLine(...)` from `status.js`; `PR #<n> ↗` linking `prUrl(board.url, pr)` (`…/issues/<m>` → `…/pull/<n>`, http(s) only, else no link). Peer view links only when the URL passes the same check.

Status line: `agentState(issue, live.agent_status)` glyph and word, then `liveTitle(host, title)` when a live title exists. Peer: `live status on peer`.

Attention: the first open attention item whose `key` equals the drawer key: `▲ <KIND> · OPEN <m>m` and its note.

PHASES: `phaseTimeline(history, issue)` → `[{phase, at, current}]`, one row per history entry (upper-cased phase, local `hh:mm`); the last row is current. No stage and no history → one current row `QUEUED`.

Fragment: `parseFragment("#token=a&issue=k")` → `Map{token→a, issue→k}` with `decodeURIComponent`; `formatFragment(map)` → `"#issue=k"`, or `""` for an empty map.

## Failure modes and edge cases

- Unknown key in `#issue=`: the drawer stays closed and the fragment is cleared once boards have loaded and no row has that key.
- Issue document request fails: the drawer keeps the last good document and shows `? LIVE VIEW UNAVAILABLE` only for the screen; the header falls back to the board row.
- Screen `503 herdr_busy` or network error: `? LIVE VIEW UNAVAILABLE`; the next tick retries.
- Tab hidden: ticks return before any request; on visible, the next tick (≤2 s) resumes. Drawer closed: no timer exists, so no screen request.
- Switching machine closes the drawer (its row is gone) and clears its timer.
- Screen text is set with `textContent`, never HTML. The token never appears in a URL the page builds.
- Overlapping ticks: a sequence number drops stale responses, and a tick is skipped while the previous one is in flight.
- Plan with 200 steps: the list scrolls inside the region.

## Acceptance criteria

- **AC-1:** Rust page tests over the embedded assets assert the drawer regions in `index.html`, every step label including `✕ FAILING` and `↺ STALE`, every notice text, the `herdr agent attach` form handling, and polling gated on `document.visibilityState` and on the open drawer; the asset route test serves the new files.
- **AC-2:** Live, against a real engine and a real herdr worker: the drawer plan equals `toolu ledger status` for that worktree, the screen text changes between polls while the agent works, and a blocked worker shows the blocked block whose copy button puts the attach command on the clipboard. Screenshots go in the PR.
- **AC-3:** By keyboard: Enter on a focused row opens its drawer; Esc closes it and focus returns to that row. Loading `/#issue=<key>` opens that drawer.
- **AC-4:** No request to `/screen` is made while the drawer is closed or the tab is hidden, observed in the browser's network log over at least 10 s each.
- **AC-5:** With a peer selected, the drawer shows the plan and the peer notice, and the network log shows no `/screen` or `/agents/live` request.
- **AC-6:** Each notice appears in its real condition: no token, a queued issue, herdr stopped (`TOOLU_EPIC_HERDR=off`), a corrupt ledger file, a peer.
- **AC-7:** `cargo xtask gate --only web` passes; step mapping, notice selection, attach command, timeline and fragment live in `js/logic/` with `node --test` cases.
- **AC-8:** `cargo xtask gate` passes.

## Acceptance evidence

- **AC-1:** `cargo test -p toolu-epic-orchestrator dashboard` — reads the real asset files; fails when a region id, label or gating expression is missing.
- **AC-2:** this branch's `target/debug/toolu epic engine` on a free port with its own resource root (`TOOLU_RESOURCE_HOME`), a registered epic whose issue record points at this worktree and at this worker's real herdr agent, and a second real herdr agent left on a host's startup trust prompt (blocked). Compare the drawer step list with `toolu ledger status` in this worktree; capture two screen polls with different text, the blocked block, and the clipboard after `[ COPY ]`. Boundary: the engine's herdr call fails → `? LIVE VIEW UNAVAILABLE`.
- **AC-3/AC-4/AC-5/AC-6:** a headless browser driven against a real `toolu epic engine` started from this branch's binary, recording request URLs; boundary: hidden tab via `visibilityState`, closed drawer, peer machine selected; notices via the real conditions named in AC-6.
- **AC-7:** `node --test` over `tests/drawer.test.js` and `tests/fragment.test.js`, using the captured `fixtures/epic/dashboard/status.json` (extended with a real `issue` document) and the five ledger statuses; `cargo xtask gate --only web`.
- **AC-8:** `cargo xtask gate` through `toolu epic job --`.

## Documentation impact

`docs/epic-status-api.md` Dashboard section (and the generated text in `status_schema.rs` if it mirrors it): the drawer, its 2 s visible-and-open screen polling, the `#issue=` / `#token=` fragment and `sessionStorage` token. `docs/epic-orchestrator/README.md` dashboard paragraph if it describes the page.

## Open Questions

None blocking. Decided: #655 owns the minimal fragment parse and token read (the screen needs it; #656 depends on #655 and adds the chip). Job rows are out of reach (none rendered). PLAN UNREADABLE sits in the PLAN region.

## Spec review (2026-10-27)

- Notices: 🟡 should-fix: the peer notice lacked the issue's `[ OPEN <NAME> DASHBOARD ↗ ]` link. Fixed in the notice table.
- Notices: 🔵 consider: `PLAN UNREADABLE` is listed by the issue among screen notices; this spec places it in the PLAN region. Kept, reason recorded in Open Questions; AC-6 still observes it in its real condition.
- Notices: 🔵 consider: a `not_running` screen reply on a launched issue has no issue text; the spec adds one line rather than reuse the queued text, which would be false.
- Acceptance evidence: 🟡 should-fix: AC-2 evidence named no runnable engine. Fixed: branch binary, own resource root, real herdr agents.

Status: Approved.
