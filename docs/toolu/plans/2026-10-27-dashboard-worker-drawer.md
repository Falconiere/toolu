# Dashboard worker drawer — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-dashboard-worker-drawer-design.md   **Topic:** worker drawer with plan, live status and screen (#655)

## Evidence and approach

Inspected `crates/epic-orchestrator/assets/dashboard/` (`index.html`, `js/app.js`, `js/api.js`, `js/view/dashboard.js`, `js/logic/status.js`, CSS), `src/dashboard.rs` (embedded asset table), `src/tests/dashboard_test.rs` (asset route and page tests, fixture capture), `src/status_api.rs` (issue document: `plan`, `phase_history`, `agent_status`), `src/status_schema.rs` (generates `docs/epic-status-api.md`), and the web gate (`crates/xtask/src/gate/web.rs`, `tooling/conventions/guardrails/web/`). Reuse `agentState`, `phaseLine`, `liveTitle`, `peerDashboardUrl` from `status.js` and the `mark`/`element` DOM helpers pattern. No engine change: every field is served.

## Workstream summary

logic modules + node tests → fixture capture of a real issue document → drawer view, CSS, page regions and app wiring → Rust page and asset tests → status API doc → live browser evidence → full gate.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"DOM-free drawer logic and fragment parsing with node tests over captured real documents","check":"node --test crates/epic-orchestrator/assets/dashboard/tests/*.test.js","ac_refs":["AC-7"],"depends_on":["s2"],"paths":["crates/epic-orchestrator/assets/dashboard/js/logic/drawer.js","crates/epic-orchestrator/assets/dashboard/js/logic/fragment.js","crates/epic-orchestrator/assets/dashboard/tests/drawer.test.js","crates/epic-orchestrator/assets/dashboard/tests/fragment.test.js","crates/epic-orchestrator/assets/dashboard/tests/status.test.js","crates/epic-orchestrator/assets/dashboard/js/logic/status.js","fixtures/epic/dashboard/status.json"],"input":"fixtures/epic/dashboard/status.json captured from running engines; the five ledger statuses plus an unknown one; screen replies text, blocked with attach and --session, not_running, available false, 401, 403; queued, done and peer issues; fragments with token and issue","model":"inherit"},
  {"id":"s2","title":"capture a real issue document and token-gated screen replies into the dashboard fixture","check":"cargo test -p toolu-epic-orchestrator dashboard_status_capture && jq -e '.issue.key and (.issue | has(\"phase_history\")) and .screen_locked.status == 401' fixtures/epic/dashboard/status.json","ac_refs":["AC-6","AC-7"],"paths":["crates/epic-orchestrator/src/tests/dashboard_capture_test.rs","crates/epic-orchestrator/src/dashboard.rs","fixtures/epic/dashboard/status.json","fixtures/epic/dashboard/README.md"],"input":"Engine::start_titled fixture engines; GET /api/v1/issues/local-658 and /api/v1/issues/local-658/screen with and without a bearer token","model":"inherit"},
  {"id":"s3","title":"drawer view, styles, page regions, api readers and app wiring (open, Esc, deep link, 2 s visible-and-open polling)","check":"cargo xtask gate --only web","ac_refs":["AC-3","AC-4","AC-5","AC-7"],"depends_on":["s1"],"paths":["crates/epic-orchestrator/assets/dashboard"],"input":"embedded dashboard assets under the pinned oxlint, oxfmt, html-validate, ast-grep, jscpd and node checks","model":"inherit"},
  {"id":"s4","title":"Rust page and asset tests: drawer regions, step labels, notice texts, attach form, polling gates, new routes served","check":"cargo test -p toolu-epic-orchestrator dashboard","ac_refs":["AC-1"],"depends_on":["s3"],"paths":["crates/epic-orchestrator/src/dashboard.rs","crates/epic-orchestrator/src/tests/dashboard_test.rs","crates/epic-orchestrator/src/tests/dashboard_drawer_test.rs","crates/epic-orchestrator/assets/dashboard"],"input":"real embedded asset bytes over a running listener and the files on disk","model":"inherit"},
  {"id":"s5","title":"status API reference documents the drawer, fragment, sessionStorage token and polling","check":"cargo test -p toolu-epic-orchestrator status_schema && cargo xtask check-markdown-cli","ac_refs":["AC-8"],"depends_on":["s3"],"paths":["crates/epic-orchestrator/src/status_schema.rs","docs/epic-status-api.md","docs/epic-orchestrator/README.md"],"input":"status_schema::render compared byte-for-byte with docs/epic-status-api.md","model":"inherit"},
  {"id":"s6","title":"live browser evidence against this branch's engine with real herdr agents","check":"for n in 2 3 4 5 6; do rg -q \"AC-$n: pass\" docs/toolu/evidence/2026-10-27-dashboard-worker-drawer.md || exit 1; done","ac_refs":["AC-2","AC-3","AC-4","AC-5","AC-6"],"depends_on":["s3","s4"],"paths":["docs/toolu/evidence/2026-10-27-dashboard-worker-drawer.md","crates/epic-orchestrator/assets/dashboard"],"input":"target/debug/toolu epic engine on a free port with its own TOOLU_RESOURCE_HOME and a peer engine; issue records pointing at this worktree and at real herdr agents (one working, one blocked on a startup prompt); headless Chrome over CDP recording request URLs; token URL; TOOLU_EPIC_HERDR=off engine; corrupt ledger copy","model":"inherit"},
  {"id":"s7","title":"full repository quality gate","check":"cargo build -p toolu-cli --bin toolu && mkdir -p /private/var/tmp/toolu-655-gate/tmp \"$HOME/.cache/toolu-655-gate/resources\" && env TMPDIR=/private/var/tmp/toolu-655-gate/tmp TOOLU_RESOURCE_HOME=\"$HOME/.cache/toolu-655-gate/resources\" PATH=\"$PWD/target/debug:$HOME/.cargo/bin:$PATH\" cargo xtask gate --root .","ac_refs":["AC-8"],"depends_on":["s4","s5","s6"],"paths":["crates/epic-orchestrator","docs/epic-status-api.md","fixtures/epic/dashboard"],"input":"whole branch","model":"inherit"}
]
```

## Critical files

- Create: `crates/epic-orchestrator/assets/dashboard/js/logic/drawer.js`, `js/logic/fragment.js`, `js/view/drawer.js`, `drawer.css`, `tests/drawer.test.js`, `tests/fragment.test.js`, `docs/toolu/evidence/2026-10-27-dashboard-worker-drawer.md`.
- Modify: `assets/dashboard/index.html`, `js/app.js`, `js/api.js`, `js/view/dashboard.js`, `src/dashboard.rs`, `src/tests/dashboard_test.rs`, `src/status_schema.rs`, `docs/epic-status-api.md`, `fixtures/epic/dashboard/status.json`.

## Verification

Node tests prove the mapping, notices, attach command, timeline and fragment over captured real documents. Rust tests prove the served bytes and page structure. A headless Chrome session against this branch's engine proves keyboard open/close, deep link, the screen updating, the blocked copy, polling gates (request log while closed and hidden) and each notice in its real condition; screenshots go to the PR. The generated status API doc stays byte-identical to `status_schema::render`. `cargo xtask gate` runs last.

## Deviations

- s2's check no longer sets `TOOLU_CAPTURE_DASHBOARD`: recapturing on every ledger run rewrote the fixture (its plan summary is this branch's live ledger) and staled the other steps. The fixture was captured once with `TOOLU_CAPTURE_DASHBOARD=1`; the check proves the capture test still passes against running engines and the committed fixture has the new fields.
- s1 runs every `tests/*.test.js`: `status.test.js` hard-coded the old capture's plan summary and now derives it from the fixture; `status.js` exports `DONE_STAGES` for the drawer.

- The first full gate failed on clippy's 50-line function limit (`dashboard::asset`, two tests) and the 300-line file limit. `dashboard.rs` now looks assets up in one table; the drawer page tests moved to `tests/dashboard_drawer_test.rs` and the fixture capture to `tests/dashboard_capture_test.rs`.

## Plan review (2026-10-27)

- Steps: 🔴 blocker: s1's node tests read the issue document that s2 captures, but s1 had no dependency. Fixed: s1 depends on s2 (s2 runs first despite its id).
- Steps: 🟡 should-fix: s7 ran the gate with the shared TMPDIR and resource home, which other worktrees' engines use. Fixed: the isolated native gate form used by #654.
- Steps: 🔵 consider: s3's check covers only the web gate; Rust route wiring is proven by s4, which depends on s3.

Status: Approved.
