# Dashboard control UI — live evidence (#656)

**Date:** 2026-10-27   **Binary:** `target/debug/toolu` built from this branch   **herdr:** 0.9.5   **Browser:** headless Google Chrome over CDP (`/private/tmp/ctl-live/run.mjs`)

## Rig

- Local engine: `toolu epic engine --http 127.0.0.1:7801` with its own `TOOLU_RESOURCE_HOME` and `TOOLU_CONFIG_DIR`, so the status token lives in a file-backed `secrets.json` (created with `toolu epic token new`) that rotation can change. `epic.peers` names `build-mini` at `127.0.0.1:7802`, a second engine with its own resource root and no token.
- Epic `live-656` (fake tracker refs, so a GitHub refresh cannot pull in real issues) with three issues:
  - `ctl-worker`: a real Claude Code agent started with `herdr agent start ctl-worker --kind claude` in a fresh folder, trust accepted through `herdr agent send-keys`. It raised `needs-human` with a real `toolu epic report needs-human`.
  - `ctl-blocked`: a real Claude Code agent left on its folder-trust prompt (`blocked`), with a real `toolu epic notify --kind blocked`.
  - `ctl-queued`: open blocker, never launched.
- Codex cooldown: an entry in the resource store's `state.json` `cooldowns` (the shape `limit.rs` writes), 41 minutes out.
- Uncertain delivery: a third engine on `127.0.0.1:7803` with `TOOLU_EPIC_SCRIPTS` whose `prompt` script prints `{"outcome":"uncertain"}`, its own token, and one reported issue `ctl-unsure` with `needs-human`.
- Every page load used the URL printed by `toolu epic token url`.

## Results

Final run, after the live-run fixes and the `/code-review xhigh --fix` round, against a rebuilt binary:

    AC-3 unlock: hash="" href-has-token=false stored=true chip=● CONTROL UNLOCKED [ LOCK ]
    AC-3 resume all: toast="✓ SENT resume all by=web The effect lands through the journal stream." overview.paused=false
    AC-3 pause epic: toast="✓ SENT pause live-656 by=web The effect lands through the journal stream." post=/api/v1/actions/pause auth=true type=application/json body={"epic":"live-656"}
    AC-3 epic paused on page: button=RESUME EPIC badge=‖PAUSED api=true
    AC-3 resume epic: toast="✓ SENT resume live-656 by=web The effect lands through the journal stream." api=false journal=["resume live-656 live-656 by=web","pause live-656 live-656 by=web"]
    AC-3 pause all: toast="✓ SENT pause all by=web The effect lands through the journal stream." overview.paused=true journal=["pause  all by=web","resume live-656 live-656 by=web"]
    AC-3 hosts before: claude="Claude Code●IN ROTATIONclaude · 1 runningReady for new launches.PAUSE" codex="Codex◷COOLINGcodex · 0 runningusage limit · back in 40:51RESUME"
    AC-3 host pause: toast="✓ SENT host-paused claude by=web The effect lands through the journal stream." row="Claude Code‖PAUSEDclaude · 1 runningpaused by web · 19:31 · 1 still runningRESUME"
    AC-3 host resume: toast="✓ SENT host-resumed claude by=web The effect lands through the journal stream." row="Claude Code●IN ROTATIONclaude · 1 runningReady for new launches.PAUSE"
    AC-3 paused+cooling: codex="Codex‖PAUSED · COOLINGcodex · 0 runningResume clears both.RESUME"
    AC-3 cooling resume: toast="✓ SENT host-resumed codex by=web The effect lands through the journal stream." codex="Codex●IN ROTATIONcodex · 0 runningReady for new launches.PAUSE" api cooldown_until=null paused=false journal="host-resumed codex by=web"
    AC-3 attention actions: ["▲ BLOCKED [ ACK ]","▲ NEEDS-HUMAN [ ANSWER ]"]
    AC-3 ack: toast="✓ SENT acknowledged ctl-blocked by=web The effect lands through the journal stream." slot="✓ ACKNOWLEDGED" api=["blocked:acknowledged","needs-human:open"]
    AC-3 drawer actions: ["ANSWER → label=Answer ctl-worker","ACK label=Acknowledge ctl-worker"]
    AC-6 dialog: {"open":true,"title":"Answer ctl-worker","status":"✓DONE","asked":"Which word should the rig worker print? Answer with one word.","warning":null,"count":"0 / 4000","send":"disabled","result":null,"focus":"answer-text","drawer":true}
    AC-6 tab cycle: ["answer-cancel","answer-text","answer-cancel","answer-text","answer-cancel","answer-text"]
    AC-6 esc: {"open":false,"title":"Answer ctl-worker","status":"✓DONE","asked":"Which word should the rig worker print? Answer with one word.","warning":null,"count":"0 / 4000","send":"disabled","result":null,"focus":"answer:ctl-worker:needs-human::drawer","drawer":true}
    AC-3 typed: {"open":true,"title":"Answer ctl-worker","status":"✓DONE","asked":"Which word should the rig worker print? Answer with one word.","warning":null,"count":"94 / 4000","send":"enabled","result":null,"focus":"answer-text","drawer":true}
    AC-3 delivered: toast="✓ DELIVERED Attention marked answered." dialog={"open":false,"title":"Answer ctl-worker","status":"✓DONE","asked":"Which word should the rig worker print? Answer with one word.","warning":null,"count":"94 / 4000","send":"enabled","result":null,"focus":"answer:ctl-worker:needs-human::drawer","drawer":true}
    AC-3 attention after answer: api=["blocked:acknowledged","needs-human:answered"] journal=["answered ctl-worker by=web","answer ctl-worker Write a numbered list of 30 short facts about plumbing pipes, then end with the word PONG-656."]
    AC-3 worker screen has the answer: true
    AC-5 working: {"open":true,"title":"Answer ctl-worker","status":"●WORKING","asked":"Which word should the rig worker print? Answer with one word.","warning":"▲ The agent is working. The host may queue this or apply it mid-turn.","count":"0 / 4000","send":"disabled","result":null,"focus":"answer-text","drawer":true}
    AC-5 blocked: {"open":true,"title":"Answer ctl-blocked","status":"▲BLOCKED","asked":"Waiting on the trust prompt.","warning":null,"count":"3 / 4000","send":"enabled","result":"● REFUSED · agent_blocked The engine refused the answer. Attach from a terminal. Engine: worker prompt not delivered: herdr agent.prompt: agent_blocked: agent ctl-blocked is blocked and requires interactive input","focus":"answer-text","drawer":true}
    AC-4 rotated: banner="● 401 · TOKEN ROTATED — Control is locked. Nothing was paused. Open the new token URL to unlock." stored=null chip="○ CONTROL LOCKED [ UNLOCK ]" posts=1 overview.paused=false toast=null
    lock: chip="○ CONTROL LOCKED [ UNLOCK ]" stored=null enabled-actions=0
    unlock: banner="○ CONTROL LOCKED — Watching only. Open the token URL the engine printed to pause, resume, answer or release. Treat that URL like a password." chip="○ CONTROL LOCKED [ UNLOCK ]"
    AC-7 peer: visible-actions=0 toggle-hidden=true slots-with-buttons=0 posts-6s=0
    AC-7 peer drawer actions=0
    AC-5 uncertain: {"open":true,"title":"Answer ctl-unsure","status":"?UNKNOWN","asked":"Proceed with the plan?","warning":null,"count":"22 / 4000","send":"disabled","result":"▲ UNCERTAIN Delivery uncertain; check the screen.","focus":"answer-text","drawer":true} attention=["needs-human:open"]
    AC-1 urls: requests=202 posts=13 with-token=0

Real mouse input (`Input.dispatchMouseEvent`, press and release on the Cursor Agent host's `PAUSE`, with the 1 s re-render in between), on the final binary including the click replay:

    hold 50 ms:   events pointerdown, mousedown, mut, mut, pointerup, mouseup, click; POSTs ["/api/v1/actions/host-pause"]; toast ✓ SENT host-paused cursor by=web
    hold 1600 ms: events pointerdown, mousedown, mut, mut, mut, pointerup, mouseup, click; POSTs ["/api/v1/actions/host-pause"]; toast ✓ SENT host-paused cursor by=web

Before the replay, the same probe sent no `POST` at either hold: the host list re-render (`mut`) detached the button between press and release, and Chrome dropped the click.

## Verdicts

- AC-3: pass — the token URL left an empty fragment, `location.href` held no token, and the token was in `sessionStorage`. Pause and resume all epics, pause and resume `live-656`, and pause and resume Claude Code each showed `✓ SENT <journal line>`, and the page then showed the new state: the epic's `‖ PAUSED` badge and `RESUME EPIC`, and the host's `‖ PAUSED` with "paused by web · 19:15 · 1 still running". The engine documents agreed (`paused`, `cooldown_until`). Codex cooling with about 40 minutes left showed `◷ COOLING` and `RESUME`. After `toolu epic hosts pause codex` it showed `‖ PAUSED · COOLING` with "Resume clears both.". `RESUME` then gave `● IN ROTATION`, `cooldown_until: null` and the journal record `host-resumed codex by=web`. `[ ACK ]` on the blocked notice showed `✓ SENT acknowledged ctl-blocked by=web` (the engine's record name) and then `✓ ACKNOWLEDGED`. `ANSWER →` in the drawer delivered a real answer: `✓ DELIVERED`, the text was on the worker's screen, the attention item became `answered`, and the journal shows `answered ctl-worker by=web`. Screenshots: [unlocked](../../epic-orchestrator/evidence/control-unlocked.png), [paused and cooling](../../epic-orchestrator/evidence/control-hosts-paused-cooling.png), [answer dialog](../../epic-orchestrator/evidence/control-answer-dialog.png).
- AC-4: pass — after `toolu epic token new`, `PAUSE ALL EPICS` sent one `POST`, got `401`, and showed `● 401 · TOKEN ROTATED — Control is locked. Nothing was paused. Open the new token URL to unlock.`. `sessionStorage` was empty, the chip read `○ CONTROL LOCKED`, and `/api/v1/overview` still read `paused: false`. [Screenshot](../../epic-orchestrator/evidence/control-401.png).
- AC-5: pass — with `ctl-worker` working, the dialog showed `● WORKING` and "▲ The agent is working. The host may queue this or apply it mid-turn.". Answering the blocked `ctl-blocked` gave `● REFUSED · agent_blocked` with the engine message `herdr agent.prompt: agent_blocked: agent ctl-blocked is blocked and requires interactive input`. The scripted uncertain engine gave `▲ UNCERTAIN — Delivery uncertain; check the screen.`, left its attention item open, and kept `SEND` disabled until the text changes. Screenshots: [working](../../epic-orchestrator/evidence/control-answer-working.png), [refused](../../epic-orchestrator/evidence/control-answer-refused.png), [uncertain](../../epic-orchestrator/evidence/control-answer-uncertain.png).
- AC-6: pass — every action button had an accessible name (`Answer ctl-worker`, `Acknowledge ctl-worker`, and so on). Six Tab presses stayed inside the dialog. Esc closed it, the drawer stayed open, and focus returned to the drawer's `ANSWER →`. After delivery, focus returned there too.
- AC-7: pass — with `build-mini` selected, no visible `button[data-action]`, no lock toggle, no buttons in any slot, no drawer actions, and 0 `POST`s in 6 s. [Screenshot](../../epic-orchestrator/evidence/control-peer.png).
- AC-1 (live complement): 202 requests, 13 of them `POST`s, and no request URL contained any of the three tokens. Each `POST` sent `Authorization: Bearer <token>` and `Content-Type: application/json`.
- Lock and unlock: `[ LOCK ]` cleared the token and left 0 enabled action buttons. `[ UNLOCK ]` showed the locked banner. [Screenshot](../../epic-orchestrator/evidence/control-locked.png).

## Fixed during the live run

- Tab could leave the modal dialog for the browser's own controls. The dialog now wraps Tab.
- A closed drawer kept its old action buttons. They were hidden, but they stayed enabled after `[ LOCK ]`. Closing the drawer now clears them.
- A toast from an earlier action stayed up under the 401 banner. Each action now hides the old toast first.
- Code review: the press-and-release probe above showed that a re-render between mousedown and mouseup dropped the click, even with slots reused. The controller now replays a click that starts and ends on the same control.
- Attention items stay listed after `ack` and `answer`, with `state` set to `acknowledged` or `answered`. The page kept offering `[ ACK ]` for them. Only open items now get an action; the others show `✓ ACKNOWLEDGED` or `✓ ANSWERED`.
- herdr 0.9.5 reported the idle Claude Code worker as `done`, and Codex reports `done` after every turn, so "agent not done" would have disabled `ANSWER` exactly when a worker waits for an answer. `liveAgent` now follows the engine's `has_live_agent`.

## Observed outside this change

- `journal_seq` on `/api/v1/overview` is the engine's token counter (`world.seq`, `logic.rs:8`), not the journal's last sequence. It read 4 while the journal was at 22. The dashboard's seven-event journal panel reads `/api/v1/events?since=journal_seq-7`, so it shows old events. The run checked the journal through `/api/v1/events` instead. The fix belongs to the status API, not to this change.
- On first start the rig engine tried to launch `ctl-worker` before the issues had a stage. The launch failed harmlessly (`herdr worktree.list: worktree path must be absolute`), and the rig then paused every epic.
