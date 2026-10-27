# Worker plan in the status API — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-worker-plan-status-api-design.md   **Topic:** Falconiere/toolu#652 — `plan` and `plan_summary` in the epic status API

## Evidence and approach
I inspected `crates/epic-orchestrator/src/status_api.rs` (`issue_document`, `route`, `document`), `status_epics.rs` (`detail`), `dispatch.rs` (`reply_status` uses `document`, so the socket and HTTP share the shape), `http_fleet.rs` (the proxy redacts the peer body), `status_schema.rs` (it generates `docs/epic-status-api.md`), `toolu_engine::ledger::io` (`ledger_path`, `read_ledger`, `state_dir_for`) and `Roots::project_state_root` (the per-host state root, with Muse at `.toolu/tmp/muse`). A real ledger (`~/w435-src/.claude/tmp/plan-ledger/*.json`) confirmed the keys `version`, `plan_doc`, `updated_at`, `next`, `summary` and `steps[]` (`id`, `title`, `status`, `activity`, `last_run`, plus the private `check`, `evidence_tail`, `diff_sha`, `input` and `retries`). Test rigs follow the `fleet_rig` and `peer_fixture` pattern: a `#[cfg(test)]` module wired from `lib.rs`.

## Workstream summary
engine `ledger_path_for` → `status_plan` projection (branch, allow-list, cache) → wiring into the issue and epic documents (HTTP, socket, peer) → generated docs → full gate.

## Steps (machine-readable)
```json
[
  {
    "id": "ledger-path-for",
    "title": "Add toolu_engine ledger_path_for(root, branch, roots); ledger_path delegates to it",
    "check": "test \"$(cargo test -p toolu-engine --lib ledger::io::tests::ledger_path_for -- --list 2>/dev/null | grep -c ': test$')\" -ge 2 && cargo test -p toolu-engine --lib ledger::io::tests::ledger_path_for",
    "paths": [
      "crates/core/engine/src/ledger/io.rs",
      "crates/core/engine/src/ledger/tests/io_test.rs"
    ],
    "ac_refs": [
      "AC-2"
    ],
    "input": "Roots for claude, codex, cursor and muse over a temp root and branch feat/x; expects <root>/.claude|.codex|.cursor/tmp/plan-ledger/feat_x.json and <root>/.toolu/tmp/muse/plan-ledger/feat_x.json, equal to ledger_path inside a real git repo"
  },
  {
    "id": "status-plan",
    "title": "status_plan module: head_branch from .git/HEAD, allow-listed plan projection, unreadable handling, mtime-keyed plan_summary cache",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib status_plan::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 8 && cargo test -p toolu-epic-orchestrator --lib status_plan::tests::",
    "paths": [
      "crates/epic-orchestrator/src/status_plan.rs",
      "crates/epic-orchestrator/src/tests/status_plan_test.rs",
      "crates/epic-orchestrator/src/tests/plan_rig_test.rs",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/core/engine/src/ledger"
    ],
    "depends_on": [
      "ledger-path-for"
    ],
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-5",
      "AC-6",
      "AC-9",
      "AC-11"
    ],
    "input": "Real git repos plus `git worktree add` worktrees; ledgers written by ledger_run and ledger_status for a two-step plan (true and false checks) for each host; garbage bytes; version 2; a 400-char title; git checkout -b other; git checkout --detach; File::set_modified for the mtime cases"
  },
  {
    "id": "wire-documents",
    "title": "Issue document gains plan, epic detail issues gain plan_summary; HTTP route, control-socket status and no sentinel leak",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib status_plan::route_tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 4 && cargo test -p toolu-epic-orchestrator --lib status_plan::route_tests:: && cargo test -p toolu-epic-orchestrator --lib status_api:: && cargo test -p toolu-epic-orchestrator --lib status_epics::",
    "paths": [
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/status_epics.rs",
      "crates/epic-orchestrator/src/status_plan.rs",
      "crates/epic-orchestrator/src/tests/status_api_test.rs",
      "crates/epic-orchestrator/src/tests/status_epics_test.rs",
      "crates/epic-orchestrator/src/tests/status_plan_test.rs",
      "crates/epic-orchestrator/src/tests/plan_rig_test.rs",
      "crates/epic-orchestrator/src/dispatch.rs",
      "crates/epic-orchestrator/src/tests/status_plan_route_test.rs"
    ],
    "depends_on": [
      "status-plan"
    ],
    "ac_refs": [
      "AC-1",
      "AC-3",
      "AC-4",
      "AC-8",
      "AC-5",
      "AC-2"
    ],
    "input": "A World issue whose meta names a real worktree with a ledger whose check, evidence_tail and input hold PLAN-SECRET-CANARY; served by start() (HTTP) and serve_with_http (control socket status with epic=<key>); missing and corrupt ledgers keep phase_history; /api/v1/epics/{id} plan_summary before and after an mtime-preserving rewrite and a touch; a muse-kind issue served over HTTP"
  },
  {
    "id": "peer-proxy",
    "title": "Peer proxy carries plan with configured secrets redacted",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib http_fleet::proxy_tests::fleet_proxy_plan -- --list 2>/dev/null | grep -c ': test$')\" -ge 1 && cargo test -p toolu-epic-orchestrator --lib http_fleet::proxy_tests::fleet_proxy_plan",
    "paths": [
      "crates/epic-orchestrator/src/tests/http_fleet_proxy_test.rs",
      "crates/epic-orchestrator/src/http_fleet.rs"
    ],
    "depends_on": [
      "wire-documents"
    ],
    "ac_refs": [
      "AC-7",
      "AC-4"
    ],
    "input": "peer_fixture::serve returning an issue document whose plan step title holds a secret configured on the proxying engine; expects the redaction marker and no secret through /api/v1/fleet/peers/{name}/issues/{key}"
  },
  {
    "id": "docs",
    "title": "Document plan, plan_summary, per-host ledger dirs and the unreadable error in the generated status API reference",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_schema::tests:: && grep -q 'plan_summary' docs/epic-status-api.md && grep -q '.toolu/tmp/muse/plan-ledger' docs/epic-status-api.md && grep -q 'unreadable' docs/epic-status-api.md",
    "paths": [
      "crates/epic-orchestrator/src/status_schema.rs",
      "crates/epic-orchestrator/src/tests/status_schema_test.rs",
      "docs/epic-status-api.md"
    ],
    "depends_on": [
      "wire-documents"
    ],
    "input": "Generated reference after TOOLU_UPDATE_DOCS=1; the check compares committed and generated text"
  },
  {
    "id": "gate",
    "title": "Full local quality gate",
    "check": "toolu epic job -- cargo xtask gate",
    "depends_on": [
      "ledger-path-for",
      "status-plan",
      "wire-documents",
      "peer-proxy",
      "docs"
    ],
    "ac_refs": [
      "AC-10"
    ],
    "input": "The whole workspace"
  }
]
```

## Critical files
- `crates/core/engine/src/ledger/io.rs`, `crates/core/engine/src/ledger/tests/io_test.rs`
- `crates/epic-orchestrator/src/status_plan.rs` (new), `src/tests/status_plan_test.rs` (new), `src/tests/status_plan_route_test.rs` (new), `src/tests/plan_rig_test.rs` (new test rig), `src/lib.rs`
- `crates/epic-orchestrator/src/status_api.rs`, `src/status_epics.rs`, and their tests
- `crates/epic-orchestrator/src/tests/http_fleet_proxy_test.rs`
- `crates/epic-orchestrator/src/status_schema.rs`, `docs/epic-status-api.md`

## Verification
End to end: real git worktrees and ledgers from `ledger_run`/`ledger_status` for all four hosts, read through a real HTTP listener and the control socket. Boundary cases: no ledger (`null`), garbage and `version: 2` (`unreadable`), detached HEAD, branch switch, unchanged versus touched mtime, a 400-character title, and secret sentinels. Docs are regenerated and checked. Delivery: `toolu ledger run <plan> --verify` passes, then the commit is scoped to these files. Per the user's override, the PR is opened, `cargo xtask gate` runs locally, then `/code-review xhigh --fix`, the gate runs again, and the PR is pushed and squash-merged with `--admin`.

## Plan review (2026-10-27)
- Steps: 🔴 blocker (fixed): plain test-name filters could pass while matching zero tests. Every behavior check now requires a minimum `--list` count first.
- wire-documents: 🟡 should-fix (fixed): AC-5 and AC-2 were proven only at the unit level. The step now asserts them through `/api/v1/epics/{id}` and a muse-kind issue over HTTP.
- docs: 🔵 consider (fixed): it cited AC-1 without proving it, so the ref is gone.
- Jev alignment noul: 0.28 before the fixes, 0.41 after (uncertain, not a rejection). The manual check found every AC-1..AC-11 mapped to a step whose check and input observe it with real data, with dependencies ordered.

**Status:** Approved
