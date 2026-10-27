# Dashboard control UI — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Claude Code (epic #649 worker)   **Topic:** Unlock, actions, host controls and the answer dialog on the epic dashboard (#656)

## Problem

The engine accepts token-gated actions (#650) and host pause (#653), and the drawer (#655) reserves an empty actions slot, but the page cannot act. An operator still needs the CLI to pause an epic, resume a cooling host, answer a worker, release a coupling hold or acknowledge attention. The page also gives no sign whether it holds the token, and a rotated token fails silently.

## Non-Goals

1. No engine or API change. Every route and field the page needs is already served (`POST /api/v1/actions/{op}`, `hosts` `paused`/`paused_by`/`paused_at`, issue `hold`).
2. No peer control. A peer view renders no action control and never sends a `POST`.
3. No token input, no `localStorage`, no remembered token across tabs.
4. No key presses, launch, kill or relaunch from the web.
5. No optimistic update: the page changes only when the journal stream refreshes it.

## Architecture

Pure rules in a new DOM-free `js/logic/control.js` (tested with `node --test`); one `postAction` in `js/api.js` (the only `fetch` caller); a controller `js/control.js` that owns the token, the pending flag, click handling, banners and the toast; `js/answer-dialog.js` for the dialog; `js/view/controls.js` that fills action slots after each render. All new files are embedded in `src/dashboard.rs` and the asset route test. Decisive trade-off: one delegated click handler over `button[data-action]` instead of a listener per button, because the board, hosts and attention lists are rebuilt every second by `renderTick`.

**Token (moves from `worker-drawer.js` to `js/control.js`).**
- On load and on `hashchange`, `takeToken(location.hash)` (existing `logic/fragment.js`) returns the token and the fragment without only the `token` key. The token goes to `sessionStorage["toolu.token"]`; the address bar is rewritten with `history.replaceState` to the remaining fragment (`#issue=<key>` stays).
- `readToken()` stays exported for the drawer's screen reads.
- `[ LOCK ]` removes the stored token. `[ UNLOCK ]` shows the locked banner only.
- Any `401` — from an action or from the screen route — removes the token and shows the 401 banner.

**Chip and banners** (`#top-controls`, new `#control-banner` under the heading):
- Unlocked: `● CONTROL UNLOCKED` + `[ LOCK ]`. Locked: `○ CONTROL LOCKED` + `[ UNLOCK ]`.
- Locked banner: "○ CONTROL LOCKED — Watching only. Open the token URL the engine printed to pause, resume, answer or release. Treat that URL like a password."
- 401 banner: "● 401 · TOKEN ROTATED — Control is locked. Nothing was paused. Open the new token URL to unlock."
- A new token (from a token URL) hides both banners.

**Buttons** (all `<button type="button" data-action=… >`, each with an accessible label naming its target):

| Place | Buttons | Op and body |
| --- | --- | --- |
| Top bar | `PAUSE ALL EPICS`, `RESUME ALL EPICS` (both always) | `pause {}`, `resume {}` |
| Epic header slot | `PAUSE EPIC` or `RESUME EPIC` by `board.paused`; the existing `‖ PAUSED` badge stays | `pause {epic}`, `resume {epic}` |
| Host row | `PAUSE` or `RESUME` (rule below) | `host-pause {host}`, `host-resume {host}` |
| Attention row slot | `[ ANSWER ]`, `[ RELEASE ]` or `[ ACK ]` by kind | dialog, `release {key}`, `ack {key}` or `ack {kind}` when keyless |
| Drawer actions | `ANSWER →`, `RELEASE HOLD` (when the issue row's `hold` field is set, #657), `ACK` (when attention open) | dialog, `release {key}`, `ack {key}` |

A host shows `RESUME` when it is paused or cooling (resume clears both, #653), else `PAUSE`.

**Host states** (`hostState`, `hostNote` in `logic/status.js`):

| Condition | Mark | Note |
| --- | --- | --- |
| paused and cooling | `‖ PAUSED · COOLING` | Resume clears both. |
| paused | `‖ PAUSED` | paused by `<paused_by>` · `<hh:mm of paused_at>` · `<running>` still running |
| cooling | `◷ COOLING` | usage limit · back in `m:ss` (unchanged) |
| otherwise | `● IN ROTATION` | Ready for new launches. (unchanged) |

`paused_by` missing reads `unknown`; an unparsable `paused_at` reads `—`.

**Kind-to-action mapping** (`attentionAction(kind)`): `needs-human`, `stuck`, `help`, `contract-changed` → `answer`; `coupling-hold` → `release`; any other kind, including unknown ones → `ack`. The row keeps the real kind name (`▲ NEEDS-HUMAN`). An `answer` or `release` item without a key falls back to `ack` by kind, since both need a key.

**Disable rules** (`controlDisabled({locked, pending, peer})` and `drawerActions(...)`):
- Every action button is disabled while locked or while a request is pending.
- `ANSWER →` and `[ ANSWER ]` are also disabled without a live agent, by the engine's own rule (`attention::has_live_agent`): the issue has launched and its stage is not `merged`, `cleaning`, `cleanup-incomplete` or `abandoned`. An agent status of `done` does not count: herdr reports it at the end of every Codex turn and, on herdr 0.9.5, of a Claude Code turn, which is exactly when a worker waits for an answer (found in the live run).
- Decision: `RELEASE HOLD` and `ACK` in the drawer do **not** need a live agent. The issue says "All of them are disabled while locked or with no live agent", but a coupling hold only exists on an issue that has not launched (`holds.rs` `coupling_ahead` requires an empty stage), so the literal reading would leave `RELEASE HOLD` disabled forever; the engine's `ack` and `release` need no agent either. Jev split 0.51/0.47 toward this reading; the code settles it.
- Peer mode renders none of these buttons, the chip shows only its state word, and the dialog never opens.

**Requests.** `actionRequest(op, body, token)` returns `{path: "/api/v1/actions/<op>", init: {method: "POST", cache: "no-store", headers: {Authorization: "Bearer <token>", "Content-Type": "application/json"}, body: JSON.stringify(body)}}`. The browser adds the same-origin `Origin`. The path never contains the token. `postAction` returns `{status, body}` and never throws for an HTTP status; a network error becomes `{status: 0, body: null}`.

**Results** (`actionResult(op, body, status, reply)` → `{kind, label, text}`):
- `200`, not `answer`: toast `✓ SENT <journal line>` — "The effect lands through the journal stream." Journal lines are the records the engine writes: `pause|resume <epic|all> by=web`, `host-paused|host-resumed <host> by=web`, `release <key> <partner> by=web` (the hold's partner, when known) and `acknowledged <key|kind> by=web`.
- `answer` `200` without `delivered: "uncertain"`: `✓ DELIVERED` — "Attention marked answered."
- `answer` `200` with `delivered: "uncertain"`: `▲ UNCERTAIN` — "Delivery uncertain; check the screen."
- `409`: `● REFUSED · <code>` — for `answer` "The engine refused the answer. Attach from a terminal.", else the engine message. `<code>` is the herdr code in `… herdr <method>: <code>: …` (for example `agent_blocked`), otherwise the whole engine error text. The dialog shows the full engine message too.
- `401`: the 401 banner (token cleared); no toast.
- Any other status or a network failure: `● FAILED · <error code or HTTP status>` — "Nothing changed on the page; the action may still apply (see the journal)."

**Answer dialog** (`<dialog id="answer-dialog" aria-labelledby="answer-title">` opened with `showModal()`, which traps focus and makes the page inert):
- Title `Answer <key>` and the agent's status chip (`agentState`).
- `THE WORKER ASKED` and the attention note (or "No open question; this sends a direction.").
- When the agent is `working`: "▲ The agent is working. The host may queue this or apply it mid-turn."
- `YOUR ANSWER` label on a `<textarea maxlength="4000">` and the counter `n / 4000`.
- `CANCEL` and `SEND ANSWER →`. Send is disabled while locked, while the trimmed text is empty and while a request is pending.
- Esc (the dialog's `cancel` event) and `CANCEL` close it; focus returns to the button that opened it (`ANSWER →` or `[ ANSWER ]`), or the drawer's `ANSWER →` when that node was re-rendered.
- Delivered: the dialog closes and the toast shows the result. Refused or uncertain: the dialog stays open with the result line (`role="status"`) and the engine message; the text is kept.

**Focus.** The 1 s re-render rebuilds hosts and attention rows. Each action button carries a stable `data-control`: `epic:<key>` and `host:<name>` for buttons that flip between pause and resume, otherwise `<op>:<target>`. A render keeps an unchanged button node in place, so a click that spans a re-render still lands, and the controls view restores focus to the button with the same id.

## Interfaces / Schema

```js
// js/logic/control.js (DOM-free)
export const TOKEN_KEY = "toolu.token";
export const TEXT_LIMIT = 4000;
export function attentionAction(kind)            // "answer" | "release" | "ack"
export function liveAgent(issue)                 // boolean
export function controlDisabled({ locked, pending })
export function drawerActions({ issue, attention, locked, pending }) // [{action, label, disabled}]
export function sendDisabled({ locked, pending, text })
export function actionRequest(op, body, token)   // {path, init}
export function journalLine(op, body)            // "pause all by=web"
export function refusalCode(error)               // "agent_blocked" | error text
export function actionResult(op, body, status, reply) // {kind, label, text}
export const LOCKED_BANNER, ROTATED_BANNER
// js/api.js
export async function postAction(op, body, token) // {status, body}
```

New DOM ids: `control-chip`, `control-toggle`, `pause-all`, `resume-all`, `control-banner`, `toast`, `answer-dialog`, `answer-title`, `answer-status`, `answer-asked`, `answer-warning`, `answer-text`, `answer-count`, `answer-result`, `answer-cancel`, `answer-send`. New CSS file `control.css` (colors from `tokens.css` only).

## Failure modes and edge cases

- **No token configured on the engine:** actions return `403 control_disabled`; the toast reads `● FAILED · control_disabled`. The chip still says unlocked because a token is stored; the operator's next step is `toolu epic token new`.
- **Token rotated:** the next action gets `401`; the token is cleared, the 401 banner shows, buttons disable, nothing is sent again.
- **Concurrent clicks:** `pending` disables every action button and Send until the reply arrives; a second click is ignored.
- **Engine busy (`503 engine_busy`):** toast `● FAILED · engine_busy` with "the action may still apply"; no retry.
- **Network failure:** status `0`, same failure toast; no retry.
- **Answer text:** `maxlength` keeps it at 4,000 characters; an empty or whitespace-only text keeps Send disabled. Textarea values use `\n`, so the engine's control-character rule passes for typed text; a pasted control character gets the engine's `400 invalid_request`, shown as `● FAILED · invalid_request`.
- **Attention item vanishes while the dialog is open:** the dialog keeps its text; the result still shows. Without a live agent, Send gets `409 no live agent for <key>` → `REFUSED`.
- **Peer selected:** the controller refuses to post (`postAction` is never called) and every slot stays empty; switching to a peer closes an open dialog.
- **Hidden tab:** nothing polls; a click still posts.
- **sessionStorage unavailable** (privacy mode throws): reads return null, so the page is locked; no error escapes.
- **Fragment with other keys:** only `token` is removed; `issue` stays.

## Acceptance criteria

- **AC-1:** The served page assets prove, by Rust page tests: the fragment is read with `takeToken` and rewritten with `history.replaceState`; the token is stored only through `sessionStorage`; `postAction` sends `Authorization: Bearer` and `Content-Type: application/json`; no `fetch`/`EventSource` URL template in any served JS contains `token`; `logic/control.js` holds the kind mapping and disable rules; no asset mentions `localStorage`.
- **AC-2:** `node --test` cases on `logic/control.js` show: each listed kind maps to its action and an unknown kind maps to `ack`; `controlDisabled`, `drawerActions` and `sendDisabled` follow the disable rules (locked, pending, no live agent, empty text, held issue without agent keeps RELEASE HOLD enabled); `actionRequest` puts the token only in the header; `actionResult` gives `DELIVERED`, `REFUSED · agent_blocked` (from `worker prompt not delivered: herdr agent.prompt: agent_blocked: …`), `REFUSED · no live agent for k` and `UNCERTAIN`; `journalLine` gives `host-resumed codex by=web`. `node --test` cases on `logic/status.js` give the four host marks and notes.
- **AC-3:** Live in headless Chrome against a real engine with a real herdr worker, after opening the URL from `toolu epic token url`: the address bar has no token; pause all, resume all, pause and resume one epic, pause and resume a host, and resume a cooling host each show `✓ SENT …` and the page then shows the new state from the journal stream; answering a live worker's `needs-human` item shows `✓ DELIVERED`, the text reaches the worker screen and the attention item closes. Resuming a host cooling with time left removes the countdown, reads `● IN ROTATION`, and the journal panel shows `host-resumed <host>` with `by=web`. A copied address bar URL holds no token.
- **AC-4:** Live: after `toolu epic token new`, `PAUSE ALL EPICS` gets `401`, the 401 banner shows, the token is gone from `sessionStorage`, and `/api/v1/overview` still reads `paused: false`.
- **AC-5:** Live: answering a working agent shows the working warning; answering a blocked agent shows `REFUSED · agent_blocked` (or the code herdr actually returns, recorded); a scripted engine whose `prompt` prints `{"outcome":"uncertain"}` shows `UNCERTAIN`.
- **AC-6:** Every new control has an accessible name (Rust page test plus html-validate); live: the dialog keeps Tab inside, Esc closes it, and focus returns to `ANSWER →`.
- **AC-7:** Live with a peer selected: no `button[data-action]` exists, the dialog cannot open, and the engine's request log / journal shows no `POST`.
- **AC-8:** `cargo xtask gate --only web`, `cargo xtask gate` and `cargo xtask check-markdown-cli` pass.

## Acceptance evidence

| AC | Real input | Expected | Check |
| --- | --- | --- | --- |
| AC-1 | The embedded asset files | Each assertion holds | `cargo test -p toolu-epic-orchestrator dashboard_control` |
| AC-2 | Real engine reply shapes from `docs/epic-status-api.md` and the herdr error format from `herdr_calls::result_of` | Listed outputs | `node --test crates/epic-orchestrator/assets/dashboard/tests/` (inside `cargo xtask gate --only web`) |
| AC-3 | `target/debug/toolu epic engine --http 127.0.0.1:<port>` with a temp `TOOLU_RESOURCE_HOME`, a registered epic, a real herdr agent, a cooldown in the shared resource store; headless Chrome over CDP | Toasts and journal-driven state changes; screenshots | Probe script output + screenshots recorded in `docs/toolu/evidence/2026-10-27-dashboard-control-ui.md` |
| AC-4 | Same rig, `toolu epic token new` | 401 banner, empty sessionStorage, overview unchanged | Same evidence file |
| AC-5 | Same rig: a working Claude Code agent, a blocked agent on its trust prompt; a second engine with `TOOLU_EPIC_SCRIPTS` whose `prompt` prints uncertain | Warning, `REFUSED · <code>`, `UNCERTAIN` | Same evidence file |
| AC-6 | `index.html`; live dialog | Labels; Tab cycle, Esc, focus target | Rust page test, html-validate, evidence file |
| AC-7 | Same rig with a peer configured (a second engine) | No action buttons, no POST in the CDP network log | Evidence file |
| AC-8 | Workspace | Green | `cargo xtask gate`, `cargo xtask check-markdown-cli` |

## Documentation impact

- `docs/epic-orchestrator/README.md`: a "Dashboard control" section — `toolu epic token url`, the fragment moving to `sessionStorage`, lock/unlock, 401 after rotation, what each button does, peer read-only, and the warning to treat the URL like a password.
- `docs/epic-status-api.md`: the dashboard paragraph mentions the controls and that the page posts only to its own engine.

## Deviations (execution)

- `liveAgent` follows the engine's `has_live_agent` instead of "stage running or awaiting_merge and agent not done" (see the disable rules); the live run showed an idle Claude Code worker reporting `done`.
- Attention items stay listed after `ack` or `answer` with `state` `acknowledged` or `answered`; they show `✓ ACKNOWLEDGED` or `✓ ANSWERED` instead of an action, and the drawer's `ACK` appears only for an open item.
- Code review (2026-10-27): stable focus ids for flipping buttons, unchanged buttons kept across renders, journal lines named as the engine writes them, the drawer and dialog pick the key's open attention item, a 403 screen read is retried with a new token, a result for a dialog closed while pending becomes a toast, Send stays disabled after an uncertain delivery until the text changes, and the drawer's issue and screen reads run in parallel.
- The CLI signs host pauses `by=user`; the hosts panel shows that as `cli`, as the issue's "paused by web|cli" asks.

## Spec review (2026-10-27)

- Acceptance criteria: 🟡 should-fix: the issue's scenarios (attention closes after an answer; cooling host reads `● IN ROTATION` with `host-resumed … by=web`; copied URL holds no token) were only implied. Fixed: AC-3 names each.
- Architecture: 🔵 consider: hold detection must use the issue row's `hold` field (#657), not only the attention item. Fixed in the drawer row.
- Jev: requirement coverage 1.58/2 and evidence completeness 1.68/2 before these fixes; after them every issue requirement, scenario and review addition maps to a section or AC by manual cross-check.

## Open Questions

None blocking. Decided above: drawer `RELEASE HOLD`/`ACK` need no live agent (hold semantics); top bar shows both pause-all and resume-all (Jev 0.71, mixed epic states); `REFUSED · <code>` uses the herdr code when present.
