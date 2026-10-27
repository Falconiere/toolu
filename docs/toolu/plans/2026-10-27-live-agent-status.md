# Live agent status endpoint — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-live-agent-status-design.md   **Topic:** `agent_status` in `World` and the status API, plus the cached `GET /api/v1/agents/live` (#651)

## Evidence and approach
I inspected these paths:
- `source_apply.rs`: `bind_or_gone`, `status_event`, `outage`, `same_agent`.
- `queue_fields::QueueState::agent_working`.
- `model::Issue::blank`.
- `status_api::{route, issue_document}` and `status_epics::detail`.
- `http.rs`: `Context`, `finite_response`, `respond`. `respond` already gates auth: free on loopback, token off loopback.
- `http_fleet::ROUTES`: `agents/live` is not in it.
- `herdr::{roundtrip, socket_path, HERDR_PROTOCOL}`.
- `source::prompt_enabled`.
- `control::{foreground_with_http, http_config}`: `TOOLU_EPIC_SCRIPTS`.
- `secrets::redact_text`.
- The `HerdrFixture` / `herdr_state` protocol-22 test server.
- `status_schema.rs`, which generates `docs/epic-status-api.md`.

Real `herdr agent list` output (herdr 0.9.5) shows `name`, `agent_status` and `terminal_title_stripped` on every row. The chosen design is in the approved spec: the limiter slot comes before the fill lock.

## Workstream summary
World status → status documents → live endpoint → schema docs → live probe and host table → gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "issues carry agent_status and agent_status_at_ms from snapshot, status events and outages", "check": "cargo test -p toolu-epic-orchestrator source_apply", "ac_refs": ["AC-1", "AC-8"], "paths": ["crates/epic-orchestrator/src/model.rs", "crates/epic-orchestrator/src/source_apply.rs", "crates/epic-orchestrator/src/queue_fields.rs", "crates/epic-orchestrator/src/tests/source_apply_test.rs"], "input": "source_fix fixture Engine with a running issue on pane w1:p1; a snapshot row with agent_status blocked; a pane.agent_status_changed event working then idle; a snapshot without the agent; Fact::Outage; boundary: an unknown status word; asserts the herdr-event journal step is queued by the same apply that changes agent_status", "model": "inherit"},
  {"id": "s2", "title": "issue document and epic issue rows expose agent_status and agent_status_at", "check": "cargo test -p toolu-epic-orchestrator status_api && cargo test -p toolu-epic-orchestrator status_epics", "ac_refs": ["AC-1"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/status_api.rs", "crates/epic-orchestrator/src/status_epics.rs", "crates/epic-orchestrator/src/tests/status_api_test.rs", "crates/epic-orchestrator/src/tests/status_epics_test.rs"], "input": "World with a registered epic dir on disk and an issue whose status changed at a known time; boundary: never-changed issue → agent_status unknown, agent_status_at null", "model": "inherit"},
  {"id": "s3", "title": "GET /api/v1/agents/live: socket-only herdr, 2 s cache, limiter of 8, unavailable reasons, redacted cut titles", "check": "cargo test -p toolu-epic-orchestrator http_live && cargo test -p toolu-epic-orchestrator http::tests", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-9"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/http_live.rs", "crates/epic-orchestrator/src/http.rs", "crates/epic-orchestrator/src/control.rs", "crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/tests/http_live_test.rs", "crates/epic-orchestrator/src/tests/http_test.rs", "crates/epic-orchestrator/src/tests/herdr_state_test.rs", "crates/epic-orchestrator/src/tests/control_test.rs", "crates/epic-orchestrator/src/tests/attention_ops_test.rs"], "input": "HerdrFixture protocol 22 with a real agent process named like the issue and a 200-char title holding the status token; fixture at protocol 23; absent socket; Herdr::Off and Herdr::Scripted; 8 held permits; listener on 0.0.0.0 with a token; a configured peer", "model": "inherit"},
  {"id": "s4", "title": "status API reference lists the route, the new issue fields and the live-status rules", "check": "cargo test -p toolu-epic-orchestrator status_schema", "ac_refs": ["AC-6"], "depends_on": ["s3"], "paths": ["crates/epic-orchestrator/src/status_schema.rs", "crates/epic-orchestrator/src/tests/status_schema_test.rs", "docs/epic-status-api.md"], "input": "the generated docs/epic-status-api.md compared with render()", "model": "inherit"},
  {"id": "s5", "title": "live probe: real herdr session and engine for AC-1/AC-3, four-host status and title table in the docs", "check": "grep -q 'AC-1: pass' docs/toolu/evidence/2026-10-27-live-agent-status.md && grep -q 'AC-3 herdr stopped: pass' docs/toolu/evidence/2026-10-27-live-agent-status.md && grep -q 'AC-3 off: pass' docs/toolu/evidence/2026-10-27-live-agent-status.md && grep -q 'Host probe' docs/epic-status-api.md && grep -q 'Cursor Agent' docs/epic-status-api.md", "ac_refs": ["AC-1", "AC-3", "AC-6"], "depends_on": ["s4"], "paths": ["docs/toolu/evidence/2026-10-27-live-agent-status.md", "docs/epic-status-api.md", "crates/epic-orchestrator/src/status_schema.rs"], "input": "isolated herdr 0.9.5 session toolu-651-probe with claude, codex, cursor-agent and muse agents; toolu epic engine (target/debug/toolu) on a temp state root", "model": "inherit"},
  {"id": "s6", "title": "full local gate", "check": "toolu epic job -- cargo xtask gate", "ac_refs": ["AC-7"], "depends_on": ["s5"], "paths": ["crates/epic-orchestrator", "docs/epic-status-api.md"], "input": "the whole branch", "model": "inherit"}
]
```

## Critical files
New: `crates/epic-orchestrator/src/http_live.rs` and `src/tests/http_live_test.rs`.

Modified: `model.rs`, `source_apply.rs`, `status_api.rs`, `status_epics.rs`, `http.rs`, `control.rs`, `status_schema.rs`, `tests/herdr_state_test.rs` (`terminal_title_stripped` rows), the test files that build `http::Config`, and `docs/epic-status-api.md`.

Keep every file under the quality-bar limit of 300 code lines; split `http_live` if needed.

## Verification
Run each step's check. The failure and boundary cases are:
- an unknown status word;
- an unlisted agent;
- an outage;
- protocol 23;
- an absent socket;
- off and scripted modes;
- full slots;
- a multibyte 200-character title;
- a secret near the cut;
- a missing token off loopback;
- the peer route returning 404.

Then run `toolu epic job -- cargo xtask gate`, `toolu ledger run <plan> --verify`, and `/code-review xhigh --fix`, as the user directed: CI is disabled.

Delivery: commit each green step and push. Open the PR against `main` with a body that starts `Closes Falconiere/toolu#651` / `Part of Falconiere/toolu#649`. Once the gate passes and every thread is resolved, merge it with `env -u GH_TOKEN gh pr merge --squash --admin`, as the user directed. Then sync the worktree with `main`.

## Plan review (2026-10-27)
- s1: 🟡 should-fix: AC-1's "within one journal event" was not asserted. Fixed: the input now asserts that the same apply queues the herdr-event journal step.
- s5: 🟡 should-fix: the check only proved that the files exist. Fixed: it now greps the per-AC pass lines the live probe records.
- s6: 🔵 consider: the gate inside a ledger check is slow. Kept, because AC-7 requires it.

**Status:** Approved

## Deviations
