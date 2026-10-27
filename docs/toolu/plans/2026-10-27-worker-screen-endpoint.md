# Worker screen endpoint — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-worker-screen-endpoint-design.md   **Topic:** local, token-gated worker screen reads (#654)

## Evidence and approach

The approved spec fixes the route, response states and 64 KiB JSON bound. #651's `http_live.rs` already has a protocol-22-only socket reader, one-second connect/read/write deadlines, a shared eight-call limiter and a real-socket fixture. #650's `http_access::authorized` and `http_secrets::LiveSecrets` provide the token boundary and rotation. `http_read.rs` owns local GET routing; `http_fleet::route_allowed` excludes an issue's `/screen` suffix. `status_schema.rs` generates the API reference. Herdr 0.9.5's schema confirms `agent.read` takes `target`, `source`, optional `format`, and no required line count.

## Workstream summary

Screen route and real-socket checks → generated API docs → isolated live-pane probe → full gate and delivery verification.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"screen route: token boundary, visible socket read, per-key cache, states, redaction, truncation and shared limiter","check":"cargo test -p toolu-epic-orchestrator http_screen && cargo test -p toolu-epic-orchestrator http_fleet_proxy","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-8"],"paths":["crates/epic-orchestrator/src/http_screen.rs","crates/epic-orchestrator/src/tests/http_screen_test.rs","crates/epic-orchestrator/src/http_live.rs","crates/epic-orchestrator/src/http.rs","crates/epic-orchestrator/src/http_read.rs","crates/epic-orchestrator/src/http_access.rs","crates/epic-orchestrator/src/http_secrets.rs","crates/epic-orchestrator/src/http_response.rs","crates/epic-orchestrator/src/http_fleet.rs","crates/epic-orchestrator/src/client.rs","crates/epic-orchestrator/src/herdr.rs","crates/epic-orchestrator/src/model.rs","crates/epic-orchestrator/src/control.rs","crates/epic-orchestrator/src/herdr_argv.rs","crates/epic-orchestrator/src/lib.rs","crates/epic-orchestrator/src/tests/herdr_state_test.rs","crates/epic-orchestrator/src/tests/herdr_fixture_test.rs","crates/epic-orchestrator/src/tests/http_fleet_proxy_test.rs"],"input":"Real TCP listener and protocol-22 Unix socket with agent process named by issue.agent; raw agent.read params assert source visible, format text and no lines; token/no-token loopback; off/scripted/absent socket/protocol 23; agent_not_found and pane_not_found; named session blocked status; two immediate same-key reads; nine concurrent stalled reads and responsive overview; configured canary; 64 KiB plus Unicode final line; peer route refuses locally","model":"inherit"},
  {"id":"s2","title":"generated status API reference documents screen route, authentication, cache and response states","check":"cargo test -p toolu-epic-orchestrator status_schema && cargo xtask check-markdown-cli","ac_refs":["AC-2","AC-4","AC-5","AC-6","AC-7"],"depends_on":["s1"],"paths":["crates/epic-orchestrator/src/status_schema.rs","crates/epic-orchestrator/src/tests/status_schema_test.rs","docs/epic-status-api.md"],"input":"status_schema::render compared byte-for-byte with generated docs/epic-status-api.md; documented token, local-only path, states and 64 KiB limit","model":"inherit"},
  {"id":"s3","title":"live isolated herdr probe with real Claude Code and Codex workers, printed canary and oversized fixture","check":"rg -q 'AC-1: pass' docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md && rg -q 'AC-3: pass' docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md && rg -q 'AC-4: pass' docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md && rg -q 'AC-5: pass' docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md && rg -q 'AC-6: pass' docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md","ac_refs":["AC-1","AC-3","AC-4","AC-5","AC-6"],"depends_on":["s1","s2"],"paths":["docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md","crates/epic-orchestrator/src/http_screen.rs","docs/epic-status-api.md"],"input":"Isolated named herdr 0.9.5 session and temporary engine/resource root; engine-started Claude Code and Codex agents during a long command; real pane prints canary and >64 KiB fixture with [server] headless_cols = 800, headless_rows = 100; close pane and turn herdr off/stopped; record response status, byte length, line count, timestamps and request params","model":"inherit"},
  {"id":"s4","title":"full repository quality gate","check":"cargo xtask gate","ac_refs":["AC-9"],"depends_on":["s3"],"paths":["crates/epic-orchestrator","docs/epic-status-api.md","docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md"],"input":"whole branch with focused real-socket tests and live-pane evidence","model":"inherit"}
]
```

## Critical files

New: `crates/epic-orchestrator/src/http_screen.rs`, `crates/epic-orchestrator/src/tests/http_screen_test.rs`, `docs/toolu/evidence/2026-10-27-worker-screen-endpoint.md`.

Modified: `http_live.rs` (make its socket helpers reusable), `http.rs`, `http_read.rs`, `control.rs`, `herdr_argv.rs`, `lib.rs`, `tests/herdr_state_test.rs`, `tests/http_fleet_proxy_test.rs`, `status_schema.rs`, `docs/epic-status-api.md`. Touch only the listed paths actually needed by the implementation; maintain the 300-code-line limit.

## Verification

Start s1 with a red real-socket/listener check, then implement and run its check to green. The test asserts the exact herdr request and refusal behavior, cache timestamp and call count, redaction, JSON byte bound and peer denial. Regenerate the API reference from `status_schema.rs` and run s2. For s3, use `.toolu/skills/verify-epic-watcher/SKILL.md` in an isolated named session and record observed commands/results, including the blocked dialog, the configured headless viewport for the 64 KiB fixture, and unavailable cases. Feasibility was checked on herdr 0.9.5: `headless_cols = 800`, `headless_rows = 100` returned 71,284 bytes from `pane read --source visible` after the real pane printed a fixture. A synthetic response alone does not satisfy the live probe. Run `toolu epic job -- cargo xtask gate`; a capacity refusal means wait and retry the same check.

Before implementation and before push, fetch and rebase if `origin/main` moved, rerunning affected checks. Commit green work promptly and push the issue branch. Finish with `toolu ledger run docs/toolu/plans/2026-10-27-worker-screen-endpoint.md --verify`, `toolu-review:review`, and `toolu ledger verdict status` at `overall: ready`. Open a PR targeting `main` with `Closes Falconiere/toolu#654` and `Part of Falconiere/toolu#649`, report its number, then hand monitoring back to the epic engine at `report babysit`.

## Plan review (2026-10-27)

- s1: 🟡 should-fix: the original `paths` omitted reused socket, auth and peer files read by its check. Added them.
- s3: 🟡 should-fix: an oversized visible viewport was assumed without setup evidence. An isolated herdr 0.9.5 headless server with `headless_cols = 800` and `headless_rows = 100` returned a 71,284-byte real visible pane; the exact configuration is now in the step.
- The `pl_check_ac_refs` helper is absent from this host. A direct JSON parse found four valid steps, all spec AC-1 through AC-9 referenced, and no dangling reference.

**Status:** Approved
