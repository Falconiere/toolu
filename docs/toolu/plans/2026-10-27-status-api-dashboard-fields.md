# Status API fields for the redesigned dashboard — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-status-api-dashboard-fields-design.md   **Topic:** Falconiere/toolu#657 — board header, row, hold, hostname and dashboard_url fields

## Evidence and approach
Inspected `status_epics.rs` (`summary`, `detail`, `issue_row`), `status_api.rs` (`document`, `route`, `issue_document` with its route-then-launch-record `model`/`effort`), `dispatch.rs` (`reply_status` calls `status_api::document`, so the socket and HTTP share documents), `fleet.rs` (`Machine`, `peer_machines`, `local_machine`, `project`), `http_fleet.rs` (proxy redacts with local secrets), `epics.rs::load_epic_state` (`world.epics[key].graph` is the parsed `graph::Graph` with `EpicInfo`), `snapshot.rs::load_issue` (every graph issue enters `World` with stage `""`), `config/epic.rs::peer_url` (refuses credentials, path, query, fragment), and the rigs `scenario_rig` (GitHub fixture + engine refresh writes `graph.json`), `fleet_rig` (real engines with socket and HTTP) and `peer_fixture`. Runtime `nix` 0.31 has `features = ["user"]`; `hostname` goes beside it.

## Workstream summary
runtime hostname → epic header and row fields → issue `hold` → scenario board test → fleet `hostname`/`dashboard_url` and proxy/socket parity → generated docs → full gate.

## Steps (machine-readable)
```json
[
  {
    "id": "runtime-hostname",
    "title": "toolu_runtime::machine::short_hostname via nix gethostname (hostname feature), cut at the first dot",
    "check": "test \"$(cargo test -p toolu-runtime --lib machine::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 2 && cargo test -p toolu-runtime --lib machine::tests::",
    "paths": [
      "crates/core/runtime/Cargo.toml",
      "crates/core/runtime/src/lib.rs",
      "crates/core/runtime/src/machine.rs",
      "crates/core/runtime/src/tests/machine_test.rs",
      "Cargo.lock"
    ],
    "ac_refs": ["AC-4"],
    "input": "The real machine's name, compared with `/bin/hostname` output cut at '.'; the cutting function on `studio-mbp.local`, `build-mini` and an empty name"
  },
  {
    "id": "epic-header-rows",
    "title": "Epic list and detail gain title, url, waves_total, current_wave, live, queued; rows gain model, effort, phase_age_ms, hold; issue document gains hold",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib status_epics::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 5 && cargo test -p toolu-epic-orchestrator --lib status_epics::tests:: && cargo test -p toolu-epic-orchestrator --lib status_api::",
    "paths": [
      "crates/epic-orchestrator/src/status_epics.rs",
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/epics.rs",
      "crates/epic-orchestrator/src/graph.rs",
      "crates/epic-orchestrator/src/graph/levels.rs",
      "crates/epic-orchestrator/src/tests/status_epics_test.rs",
      "crates/epic-orchestrator/src/tests/status_api_test.rs"
    ],
    "depends_on": ["runtime-hostname"],
    "ac_refs": ["AC-2", "AC-3", "AC-6"],
    "input": "graph.json written by graph::write_graph (support::graph_248 shape) with epic.title/url, waves 1..3, loaded into World by snapshot; stages merged/abandoned/running/'' set on World issues; a 200-char title holding a secret from TOOLU_EPIC_PEER_TOKENS loaded by secrets::load; route and launch records with model/effort; a launch record with hold {kind, with, score, source, at}. Boundaries: a registered epic with no graph.json (title/url/current_wave null, waves_total 0); every issue merged or abandoned (current_wave null); an abandoned wave-1 issue does not pin wave 1; a title of exactly 160 chars unchanged; a row with no route or launch record has model/effort null; score and source absent from hold"
  },
  {
    "id": "board-scenario",
    "title": "Scenario: engine-built graph.json, real engine launches and merges; header moves wave 1 -> 2 -> null and rows match the issue documents",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib status_epics::board_tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 1 && cargo test -p toolu-epic-orchestrator --lib status_epics::board_tests::",
    "paths": [
      "crates/epic-orchestrator/src/status_epics.rs",
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/epics.rs",
      "crates/epic-orchestrator/src/graph/levels.rs",
      "crates/epic-orchestrator/src/tests/status_epics_board_test.rs",
      "crates/epic-orchestrator/src/tests/scenario_rig_test.rs"
    ],
    "depends_on": ["epic-header-rows"],
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "input": "scenario_rig::Scenario::new(&[(2, &[]), (3, &[]), (4, &[2]), (5, &[4])], 2): GitHub fixture epic o/r#1 titled 'Epic'; engine pump launches 2 and 3; close + report ready merges them; then 4 and 5"
  },
  {
    "id": "fleet-fields",
    "title": "Overview and fleet machines gain hostname; peers gain dashboard_url; socket status and peer proxy carry the new fields",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib http_fleet:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 10 && cargo test -p toolu-epic-orchestrator --lib http_fleet:: && cargo test -p toolu-epic-orchestrator --lib fleet::tests:: && cargo test -p toolu-runtime --lib config::epic::tests::",
    "paths": [
      "crates/epic-orchestrator/src/fleet.rs",
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/tests/fleet_test.rs",
      "crates/epic-orchestrator/src/tests/http_fleet_test.rs",
      "crates/epic-orchestrator/src/tests/http_fleet_proxy_test.rs",
      "crates/epic-orchestrator/src/tests/http_fleet_board_test.rs",
      "crates/epic-orchestrator/src/http_fleet.rs",
      "crates/epic-orchestrator/src/tests/fleet_rig_test.rs",
      "crates/core/runtime/src/config/tests/epic_test.rs"
    ],
    "depends_on": ["epic-header-rows"],
    "ac_refs": ["AC-4", "AC-5", "AC-8", "AC-9"],
    "input": "Two real engines (fleet_rig::Engine) A with peer B at http://127.0.0.1:<port>/; /bin/hostname as oracle; socket op status epic=<key> vs GET /api/v1/epics/<key>; B's graph.json epic.title holding a secret configured on A via TOOLU_EPIC_PEER_TOKENS; config::epic::parse on peers url http://user:pass@10.0.0.2:7717/?x=1#y (refused, error without user:pass) and http://10.0.0.2:7717/ (dashboard_url equal). Boundaries: local entry dashboard_url null; unreachable peer keeps dashboard_url and has hostname null"
  },
  {
    "id": "docs",
    "title": "Generated status API reference lists the new fields and a board header paragraph",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_schema::tests:: && grep -q 'current_wave' docs/epic-status-api.md && grep -q 'dashboard_url' docs/epic-status-api.md && grep -q 'hostname' docs/epic-status-api.md",
    "paths": [
      "crates/epic-orchestrator/src/status_schema.rs",
      "crates/epic-orchestrator/src/tests/status_schema_test.rs",
      "docs/epic-status-api.md"
    ],
    "depends_on": ["fleet-fields", "board-scenario"],
    "input": "Generated reference after TOOLU_UPDATE_DOCS=1; the check compares committed and generated text"
  },
  {
    "id": "gate",
    "title": "Full quality gate (cargo xtask gate in the clean container via deploy-gate)",
    "check": "c=$(jq -r .commit target/deploy/gate.json) && git merge-base --is-ancestor \"$c\" HEAD && git diff --quiet \"$c\" HEAD -- . ':!docs/toolu'",
    "depends_on": ["runtime-hostname", "epic-header-rows", "board-scenario", "fleet-fields", "docs"],
    "ac_refs": ["AC-7"],
    "input": "The receipt `cargo xtask deploy-gate` writes after the full `cargo xtask gate` passes in a clean container, for a commit whose tree equals HEAD outside docs/toolu"
  }
]
```

## Critical files
- `crates/core/runtime/Cargo.toml`, `src/lib.rs`, `src/machine.rs` (new), `src/tests/machine_test.rs` (new), `Cargo.lock`
- `crates/epic-orchestrator/src/status_epics.rs`, `src/status_api.rs`, `src/fleet.rs` and their tests; `src/tests/status_board_test.rs` (new, wired from `status_epics.rs`)
- `crates/core/runtime/src/config/tests/epic_test.rs`
- `crates/epic-orchestrator/src/status_schema.rs`, `docs/epic-status-api.md`

## Verification
End to end: an engine-built `graph.json` from the GitHub fixture, a real engine launching and merging, real HTTP listeners and the control socket on two engines, `/bin/hostname` as the oracle. Boundaries: abandoned issue, every issue merged (`null`), missing route record, epic without a parsed graph, 160 vs 200-character titles, secret sentinels, credentialed peer URL refused. Docs regenerated and compared. Delivery: `toolu ledger run <plan> --verify`, scoped commit, review, verdict ready, push.

## Plan review (2026-10-27)
- fleet-fields: 🔴 blocker (fixed): the config test filter `config::tests::epic` matches nothing; the module path is `config::epic::tests::`.
- epic-header-rows, fleet-fields: 🟡 should-fix (fixed): the spec's boundary cases (no graph, all merged, abandoned, 160-character title, missing records, local `dashboard_url`, unreachable peer) were not in any step input. Added.
- AC coverage: AC-1..AC-9 all mapped (`jq` over `ac_refs`).
- Jev: dependency order 0.74; AC/step alignment 0.36 → 0.33 (uncertain, not a rejection). Manual check: every AC maps to a step whose check runs a counted test filter over the named real input, and AC-7 is the container gate receipt, as in the #652 plan.

**Status:** Approved

## Deviations
- epic-header-rows: the production change was written before its tests in this step; red was then shown by running the new tests against the `HEAD` versions of `status_epics.rs` and `status_api.rs` (5 of 5 failed) before restoring them.
- board-scenario: the scenario showed the graph renumbers waves over open issues after each refresh (`[2, 1, 1, 1]` instead of `[3, 2, 1, 1]` after wave 1 merged). `status_epics::stable_waves` now levels every issue; the epic #248 fixture expectations moved to 6 waves, checked by hand against its blockers.
- fleet-fields: the hostname, dashboard URL and socket-parity tests live in a new `tests/http_fleet_board_test.rs` (`http_fleet::board_tests`), since `http_fleet_test.rs` would pass 300 code lines. `fleet_rig::Engine::start_titled` writes a complete graph (so the engine parses its title) and pauses the epic: with a parsed graph the engine journals `launch-intent` before the worker's report, and the report then no longer adopts the worker as `running`.
- After `/code-review xhigh --fix` (2026-10-27): the review fixed a stale cycle between closed issues (it falls back to the open waves). Its other findings were fixed as listed in the spec's "Changes after code review". The scenario test moved to `tests/status_epics_board_test.rs`, and `epics.rs` and `graph/levels.rs` joined the step paths.
