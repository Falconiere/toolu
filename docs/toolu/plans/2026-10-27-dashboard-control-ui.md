# Dashboard control UI — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-dashboard-control-ui-design.md   **Topic:** Unlock, actions, host controls and the answer dialog on the epic dashboard (#656)

## Evidence and approach

Inspected: `assets/dashboard/js/{app,api,worker-drawer}.js`, `js/logic/{fragment,status,drawer}.js`, `js/view/{dashboard,drawer,peer}.js`, `index.html`, `src/dashboard.rs` (embedded asset list), `src/tests/dashboard_{test,drawer_test}.rs` (string page tests and served-route test), `src/http_action.rs` (op allow-list incl. `host-pause`/`host-resume`, 409 `{"error"}`), `src/attention_ops.rs` (answer refusals and `delivered: "uncertain"`), `src/herdr_calls.rs` (`herdr <method>: <code>: <message>`), `src/holds.rs` (holds only on unlaunched issues), `docs/epic-status-api.md` (generated from `src/status_schema.rs`), and the #655 plan and live browser rig memory.

Approach: pure rules in `js/logic/control.js`; `postAction` in `api.js`; controller `js/control.js` (token, pending flag, delegated clicks, chip, banners, toast); `js/answer-dialog.js` (native `<dialog>` + `showModal`); `js/view/controls.js` fills the existing epic/attention slots plus new host and drawer slots after every render and restores focus by `data-control`. Host marks move into `logic/status.js`. Token storage moves out of `worker-drawer.js` into `control.js`; the screen-route 401 goes through the same `rotated()` path.

## Workstream summary

logic + node tests → host marks → page (HTML, CSS, controller, dialog, slots, api) → Rust page tests and asset routes → docs → live browser evidence → full gate.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"DOM-free control rules: kind mapping, disable rules, request shape, journal line, refusal code, result text, with node tests","check":"node --test crates/epic-orchestrator/assets/dashboard/tests/*.test.js","ac_refs":["AC-2"],"paths":["crates/epic-orchestrator/assets/dashboard/js/logic/control.js","crates/epic-orchestrator/assets/dashboard/tests/control.test.js","crates/epic-orchestrator/assets/dashboard/js/logic/status.js","crates/epic-orchestrator/assets/dashboard/tests/status.test.js"],"input":"engine reply shapes from docs/epic-status-api.md (200 {ok}, 200 delivered uncertain, 409 {schema,error}); herdr error text 'worker prompt not delivered: herdr agent.prompt: agent_blocked: …' as herdr_calls::result_of formats it; host items with paused/paused_by/paused_at/cooldown_until; every engine attention kind plus an unknown one","model":"inherit"},
  {"id":"s2","title":"page controls: chip, banners, toast, top-bar/epic/host/attention/drawer buttons, answer dialog, postAction, token move, focus restore","check":"cargo xtask gate --only web","ac_refs":["AC-6","AC-7","AC-8"],"depends_on":["s1"],"paths":["crates/epic-orchestrator/assets/dashboard"],"input":"embedded dashboard assets under the pinned oxlint, oxfmt, html-validate, ast-grep, jscpd and node checks","model":"inherit"},
  {"id":"s3","title":"Rust page tests and served routes for the new assets: token rules, headers, no token in URLs, no localStorage, labels, mapping","check":"cargo test -p toolu-epic-orchestrator dashboard","ac_refs":["AC-1","AC-6"],"depends_on":["s2"],"paths":["crates/epic-orchestrator/src/dashboard.rs","crates/epic-orchestrator/src/tests/dashboard_test.rs","crates/epic-orchestrator/src/tests/dashboard_control_test.rs","crates/epic-orchestrator/src/tests/dashboard_drawer_test.rs","crates/epic-orchestrator/assets/dashboard"],"input":"real embedded asset bytes over a running listener and the files on disk","model":"inherit"},
  {"id":"s4","title":"docs: README dashboard control section and status API dashboard paragraph","check":"cargo test -p toolu-epic-orchestrator status_schema && cargo xtask check-markdown-cli","ac_refs":["AC-8"],"depends_on":["s2"],"paths":["crates/epic-orchestrator/src/status_schema.rs","docs/epic-status-api.md","docs/epic-orchestrator/README.md"],"input":"status_schema::render compared byte-for-byte with docs/epic-status-api.md; README commands checked against the real toolu tree","model":"inherit"},
  {"id":"s5","title":"live browser evidence against this branch's engine with real herdr agents, a cooling host, token rotation, an uncertain scripted engine and a peer","check":"for n in 3 4 5 6 7; do rg -q \"AC-$n: pass\" docs/toolu/evidence/2026-10-27-dashboard-control-ui.md || exit 1; done","ac_refs":["AC-3","AC-4","AC-5","AC-6","AC-7"],"depends_on":["s2","s3"],"paths":["docs/toolu/evidence/2026-10-27-dashboard-control-ui.md","crates/epic-orchestrator/assets/dashboard"],"input":"target/debug/toolu epic engine on a free port with its own TOOLU_RESOURCE_HOME, registered epic whose issues are keyed after real herdr agents (one working, one blocked on its trust prompt), a cooldown in the shared resource store, toolu epic token url and token new; a second engine with TOOLU_EPIC_SCRIPTS whose prompt prints uncertain; a peer engine; headless Chrome over CDP recording network requests","model":"inherit"},
  {"id":"s6","title":"full repository quality gate","check":"cargo build -p toolu-cli --bin toolu && mkdir -p /private/var/tmp/toolu-656-gate/tmp \"$HOME/.cache/toolu-656-gate/resources\" && env TMPDIR=/private/var/tmp/toolu-656-gate/tmp TOOLU_RESOURCE_HOME=\"$HOME/.cache/toolu-656-gate/resources\" PATH=\"$PWD/target/debug:$HOME/.cargo/bin:$PATH\" cargo xtask gate --root .","ac_refs":["AC-8"],"depends_on":["s3","s4","s5"],"paths":["crates/epic-orchestrator","docs/epic-status-api.md","docs/epic-orchestrator/README.md","docs/toolu"],"input":"whole branch","model":"inherit"}
]
```

## Critical files

- Create: `crates/epic-orchestrator/assets/dashboard/js/logic/control.js`, `tests/control.test.js`, `js/control.js`, `js/answer-dialog.js`, `js/view/controls.js`, `control.css`, `crates/epic-orchestrator/src/tests/dashboard_control_test.rs`, `docs/toolu/evidence/2026-10-27-dashboard-control-ui.md`.
- Modify: `index.html`, `js/app.js`, `js/api.js`, `js/worker-drawer.js`, `js/logic/status.js`, `tests/status.test.js`, `js/view/dashboard.js` (slot data attributes only), `src/dashboard.rs`, `src/tests/dashboard_test.rs`, `src/status_schema.rs`, `docs/epic-status-api.md`, `docs/epic-orchestrator/README.md`.

## Verification

Node tests prove every rule on real reply shapes; Rust page tests prove the token, header and storage rules on the served bytes; the live run proves each action's effect arrives through the journal stream, 401 after rotation leaves the engine unchanged, and peer mode sends no `POST`. Docs stay synchronized through the `status_schema` byte comparison and `check-markdown-cli`. The full gate runs last.

## Plan review (2026-10-27)

- s5: 🔵 consider: the check reads recorded pass lines, not the browser itself. Accepted: the live run needs real herdr agents and Chrome, which a ledger check cannot start; the evidence file records the CDP output and screenshots, as #655 did.
- AC coverage: every spec AC (AC-1 to AC-8) has a step; docs have s4 with a runnable check; the full gate is last.
- Jev step/AC alignment 1.67/2; the remaining weakness is the s5 note above.
