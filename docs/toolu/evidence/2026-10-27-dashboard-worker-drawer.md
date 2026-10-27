# Dashboard worker drawer — live evidence (#655)

**Date:** 2026-10-27   **Binary:** `target/debug/toolu` built from this branch   **herdr:** 0.9.5   **Browser:** headless Google Chrome over CDP (`/private/tmp/drawer-live/run.mjs`)

## Rig

- Local engine: `toolu epic engine --http 127.0.0.1:7791` with its own `TOOLU_RESOURCE_HOME`, `TOOLU_EPIC_STATUS_TOKEN` set, and `epic.peers` naming a peer `build-mini` at `127.0.0.1:7792` (a second engine, own resource root, no token).
- Epic `live-649` (paused, so the engine launches nothing) with four issues:
  - `falconiere-toolu-655`: this worker's real herdr agent (Claude Code, working) and this worktree, so its plan is this branch's real ledger.
  - `drawer-blocked`: a real Claude Code agent started with `herdr agent start drawer-blocked --kind claude` in a new folder, left on its folder-trust prompt (`blocked`).
  - `drawer-queued`: never reported, open blocker `Falconiere/toolu#655`.
  - `drawer-corrupt`: a worktree whose branch ledger file is `{not json`.
- Each launched issue got a real `toolu epic report execution`.

## Results

    AC-3 token-url: hash="" token=stored open=false
    AC-4 closed 10s: screen requests=0, live requests=4
    AC-3 enter: open=true label=Worker falconiere-toolu-655 hash=#issue=falconiere-toolu-655 current=falconiere-toolu-655
    AC-2 plan: 5/7 GREEN | NEXT DOM-free drawer logic and fragment parsing with node tests over captured real documents | steps=["✓GREENDOM-free drawer logic and fragment parsing with node tests over captured real documents","✓GREENcapture a real issue document and token-gated screen replies into the dashboard fixture","✓GREENdrawer view, styles, page regions, api readers and app wiring (open, Esc
    AC-2 status: ●WORKING | meta=POLLED 2s · 18:29:50
    AC-2 screen changed between polls: true (lines 61 -> 61) meta=POLLED 2s · 18:29:53
    AC-2 phases: ["EXECUTION18:23 *"]
    AC-4 hidden 10s: visibility=hidden screen requests=0
    AC-4 visible again 4.5s: visibility=visible screen requests=4
    AC-3 esc: open=false focus=falconiere-toolu-655 hash=""
    AC-3 deep link: open=true label=Worker drawer-blocked
    AC-2 blocked: blocked=true attach=$ herdr agent attach drawer-blocked status=▲BLOCKEDsession claude /p/t/d/blocked-wt2
    AC-2 copy: button=COPIED ✓ clipboard=herdr agent attach drawer-blocked
    AC-6 drawer-queued: notice=/ AGENT NOT RUNNING — Queued until #655 merges. The screen appears once a host claims it. note=No plan yet. Phase history below until the worker writes its ledger. phases=["QUEUED— *"]
    AC-6 drawer-corrupt: notice=/ AGENT NOT RUNNING — The agent pane is closed. Plans and phases still update. note=●PLAN UNREADABLE The ledger failed to parse. Phase history shown instead. phases=["EXECUTION18:23 *"]
    AC-6 no token: notice=○ CONTROL LOCKED — Screen reads need the token. Open the token URL the engine printed.
    AC-5 peer: open=true notice=◇ OPEN DASHBOARD TO WATCH OR CONTROL — Screens, live status and actions stay on the peer. This view shows the plan only. link=[ OPEN BUILD-MINI DASHBOARD ↗ ] meta=LOCAL ONLY plan=5/7 GREEN steps=7
    AC-5 peer requests 6s: screen=0 live=0 peer-issue=4
    AC-2 blocked (fresh agent, engine re-seeded): AC-2 copy: button=COPIED ✓ clipboard=herdr agent attach drawer-blocked
    AC-6 herdr off: notice=? LIVE VIEW UNAVAILABLE — herdr not reachable. Plans and phases still update. plan=5/7 GREEN hash=#issue=falconiere-toolu-655

Plan comparison (the drawer renders `plan.steps` from `/api/v1/issues/falconiere-toolu-655`):

    toolu ledger status --json: {"next":"s1","steps":[{"id":"s1","status":"green"},{"id":"s2","status":"green"},{"id":"s3","status":"green"},{"id":"s4","status":"green"},{"id":"s5","status":"green"},{"id":"s6","status":"pending"},{"id":"s7","status":"pending"}]}
    drawer source:              {"next":"s1","steps":[{"id":"s1","status":"green"},{"id":"s2","status":"green"},{"id":"s3","status":"green"},{"id":"s4","status":"green"},{"id":"s5","status":"green"},{"id":"s6","status":"pending"},{"id":"s7","status":"pending"}]}

## Verdicts

- AC-2: pass — the drawer's 7 steps and `NEXT` equal `toolu ledger status` for this worktree; the screen text changed between two polls 4.5 s apart while the agent worked (`POLLED 2s` time advanced); the blocked worker showed `▲ BLOCKED · WAITING ON A KEY PRESS` and `$ herdr agent attach drawer-blocked`, and `[ COPY ]` turned into `COPIED ✓` with exactly that command on the clipboard. Screenshots: [working](../../epic-orchestrator/evidence/drawer-working.png), [blocked](../../epic-orchestrator/evidence/drawer-blocked.png).
- AC-3: pass — loading `/#token=…` left an empty fragment and stored the token; Enter on the focused row opened `Worker falconiere-toolu-655` and set `#issue=falconiere-toolu-655`; Esc closed it, cleared the fragment and focus returned to that row; loading `/#issue=drawer-blocked` opened that drawer.
- AC-4: pass — 0 screen requests over 10 s with the drawer closed, and 0 over 10 s with the window minimized (`visibilityState: hidden`); requests resumed once visible.
- AC-5: pass — with `build-mini` selected, the peer worker's drawer showed its plan (5/7 GREEN, 7 steps), `LOCAL ONLY` and the peer notice with `[ OPEN BUILD-MINI DASHBOARD ↗ ]`; over 6 s the page made 0 `/screen` and 0 `/agents/live` requests and 4 proxied `issues/peer-655` reads. [Screenshot](../../epic-orchestrator/evidence/drawer-peer.png).
- AC-6: pass — no token: `○ CONTROL LOCKED`; queued issue: `/ AGENT NOT RUNNING` with "Queued until #655 merges." and phase `QUEUED`; herdr stopped (`TOOLU_EPIC_HERDR=off` engine): `? LIVE VIEW UNAVAILABLE` while the plan still showed 5/7; corrupt ledger: `● PLAN UNREADABLE` with phase history; peer: the peer notice. Screenshots: [locked](../../epic-orchestrator/evidence/drawer-locked.png), [queued](../../epic-orchestrator/evidence/drawer-queued.png), [herdr off](../../epic-orchestrator/evidence/drawer-herdr-off.png), [corrupt](../../epic-orchestrator/evidence/drawer-corrupt.png).

## Observed outside this change

A herdr agent created after the engine's `session.snapshot` kept the issue's earlier `agent_status` (`unknown`, later `idle` from a closed pane) until the engine restarted, so the screen route served no `attach` for it. The drawer follows the engine's status; the subscription behavior belongs to #651.
