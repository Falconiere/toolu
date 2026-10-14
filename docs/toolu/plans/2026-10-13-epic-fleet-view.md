# Epic fleet view — Plan

**Date:** 2026-10-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-13-epic-fleet-view-design.md   **Topic:** Issue #453, fleet status across engines

## Evidence and approach

Inspected `crates/epic-orchestrator/src/{http,http_access,http_response,status_api,status_schema,dashboard,control,query,verbs}.rs`, `src/tests/http_test.rs` (the in-process `serve_with_http` fixture of `dashboard_live`), `crates/core/runtime/src/config/{epic,secrets}.rs` (`Peer`, `peer_token`, `TOOLU_EPIC_PEER_TOKENS`) and `crates/core/http/src/{lib,send}.rs` (`Client::send`, `Auth::Bearer`). The design follows the spec: `fleet.rs` fetches and projects, `http_fleet.rs` serves and caches, `fleet_cli.rs` drives `--fleet`, `dashboard.rs` renders. `json_response` moves to `http_response.rs` and `status_command` to `query.rs` to keep `http.rs` and `verbs.rs` under 300 code lines. Tests use real loopback listeners and in-process engines; no mocking crate.

## Workstream summary

fleet core (fetch, classify, project) → listener route, round cache and refresher → peer proxy → CLI flag → dashboard and Chrome evidence → generated docs and prose → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "fleet.rs: peer fetch with 2 s deadline, state and reason classification, allow-listed projection, summary and redaction; peer_fixture test helper. Tests: fleet::tests::{fleet_classifies_replies, fleet_peer_states_over_http, fleet_projection_drops_hostile_fields, fleet_summary_sums_ok_machines, fleet_without_peers_lists_local}",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- fleet::tests:: && cargo xtask gate --only fmt --only clippy --only guardrails",
    "ac_refs": [
      "AC-3",
      "AC-10"
    ],
    "paths": [
      "crates/epic-orchestrator/src/fleet.rs",
      "crates/epic-orchestrator/src/tests/fleet_test.rs",
      "crates/epic-orchestrator/src/tests/peer_fixture_test.rs",
      "crates/epic-orchestrator/src/lib.rs"
    ],
    "input": "loopback fixture peers answering schema 1, schema 2, text/plain 200, 404, 401, 403 foreign_host, another 403, 500, 302 and a 1.5 MiB body; a closed port; a hostile schema-1 document with unknown fields, a 300-character phase, wrong-typed counts and a non-array issues; an empty peer list",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "http_fleet.rs and http.rs hook: /api/v1/fleet with completion-timed coalescing round, Refresher (first round at start), last-seen; http::Config peers from epic.peers; json_response moved; 502 reason; every Config literal gains peers. Tests: http_fleet::tests::{fleet_two_engines, fleet_slow_peers, fleet_token_canary_fleet, fleet_last_seen, fleet_refresher_first_round, fleet_round_coalesces}",
    "check": "cargo test -p toolu-epic-orchestrator --lib && cargo xtask gate --only fmt --only clippy --only guardrails",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-4",
      "AC-5"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "crates/epic-orchestrator/src/http_fleet.rs",
      "crates/epic-orchestrator/src/tests/http_fleet_test.rs",
      "crates/epic-orchestrator/src/tests/fleet_rig_test.rs",
      "crates/epic-orchestrator/src/tests/peer_fixture_test.rs",
      "crates/epic-orchestrator/src/http.rs",
      "crates/epic-orchestrator/src/tests/http_test.rs",
      "crates/epic-orchestrator/src/tests/http_access_test.rs",
      "crates/epic-orchestrator/src/tests/http_metrics_test.rs",
      "crates/epic-orchestrator/src/http_response.rs",
      "crates/epic-orchestrator/src/tests/http_response_test.rs",
      "crates/epic-orchestrator/src/control.rs",
      "crates/epic-orchestrator/src/tests/control_test.rs",
      "crates/epic-orchestrator/src/tests/attention_ops_test.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/fleet.rs",
      "crates/epic-orchestrator/src/lib.rs"
    ],
    "input": "two in-process engines (serve_with_http) each with a registry, graph and an issue made running by a real report; engine B bound 0.0.0.0 behind a status token; a never-accepting TcpListener; a wrong configured token; engine B stopped after one ok read; a fixture peer counting overview requests with no client traffic, under 5 simultaneous requests, and answering after 1.5 s; canary peer tokens embedded in fixture replies, scanned in fleet bodies and every file under A's resource root",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Peer proxy routes in http_fleet.rs (counter inside FleetCache): split before decode, route allow-list, 8-call limit, 502/503 codes. Second test module `proxy_tests` (tests/http_fleet_proxy_test.rs): http_fleet::proxy_tests::{fleet_proxy_documents, fleet_proxy_rejects_paths, fleet_proxy_limit, fleet_token_canary_proxy}",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- http_fleet::proxy_tests:: && cargo xtask gate --only fmt --only clippy --only guardrails",
    "ac_refs": [
      "AC-6",
      "AC-4"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/epic-orchestrator/src/http_fleet.rs",
      "crates/epic-orchestrator/src/tests/http_fleet_proxy_test.rs",
      "crates/epic-orchestrator/src/tests/fleet_rig_test.rs",
      "crates/epic-orchestrator/src/tests/peer_fixture_test.rs",
      "crates/epic-orchestrator/src/fleet.rs",
      "crates/epic-orchestrator/src/http_response.rs"
    ],
    "input": "engine B as peer server: epics/{id} and issues/{key}; a percent-encoded peer name; unknown peer, disallowed route, '..', a 129-character key (404) and a 128-character key (proxied); a blackholed peer (502) and 9 concurrent proxy calls against it (503 proxy_limit) while /api/v1/overview answers 200; a canary-echoing fixture document through the proxy",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "fleet_cli.rs and query.rs: toolu epic status --fleet (conflicts with EPIC), status_command moved beside status. Tests: fleet_cli::tests::{fleet_cli_lists_peers, fleet_cli_conflicts_with_epic, fleet_cli_rejects_credential_config, fleet_token_canary_cli}. The commands snapshot in crates/cli/tests changes and is accepted in S6.",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- fleet_cli::tests:: query::tests:: verbs::tests:: && cargo xtask gate --only fmt --only clippy --only guardrails",
    "ac_refs": [
      "AC-7",
      "AC-4"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/epic-orchestrator/src/fleet_cli.rs",
      "crates/epic-orchestrator/src/tests/fleet_cli_test.rs",
      "crates/epic-orchestrator/src/query.rs",
      "crates/epic-orchestrator/src/tests/query_test.rs",
      "crates/epic-orchestrator/src/verbs.rs",
      "crates/epic-orchestrator/src/tests/verbs_test.rs",
      "crates/epic-orchestrator/src/fleet.rs",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/control.rs",
      "crates/epic-orchestrator/src/tests/fleet_rig_test.rs",
      "crates/epic-orchestrator/src/tests/peer_fixture_test.rs"
    ],
    "input": "a HOME with toolu.config.json epic.peers naming a live in-process engine, a closed port and a peer named local, TOOLU_EPIC_PEER_TOKENS with a canary echoed by a fixture peer; an empty local registry (engine-down entry); `status --fleet EPIC` through command().try_get_matches_from (usage error); a credential key under epic (exit 1)",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "dashboard.rs: machine switcher, fleet summary, sequential peer panel fetches through the proxy, 5 s polling, state banners. Test dashboard::tests::fleet_dashboard (tests/dashboard_test.rs). Evidence script docs/toolu/evidence/epic-fleet-dashboard.sh: free ports, trap kills both engines on every exit, fails clearly without Chrome, saves epic-fleet-dashboard.html; force-added",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- dashboard::tests::fleet_dashboard && cargo build -p toolu-cli && bash docs/toolu/evidence/epic-fleet-dashboard.sh",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "S3",
      "S4"
    ],
    "paths": [
      "crates/epic-orchestrator/src",
      "crates/cli/src",
      "docs/toolu/evidence/epic-fleet-dashboard.sh"
    ],
    "input": "GET / from a live listener (element ids, fleet endpoints, no external URL); two real target/debug/toolu epic serve processes plus a closed-port peer and a wrong-token peer rendered by headless Chrome: A shows local, server, Unreachable \u00b7 never seen, Unauthorized: token rejected and the fleet summary; B hides the switcher",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Docs: generated status API reference (run once with TOOLU_UPDATE_DOCS=1) with fleet routes and the Fleet over a private network section (asserted phrases in status_schema_test), config.md peer example and Host rule, plugin command/skill/README, AGENTS.md key files; regenerate docs/cli (cargo xtask docs-cli) and accept the commands snapshot (INSTA_UPDATE=always cargo test -p toolu-cli --test contract) before the check",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- status_schema::tests:: && cargo xtask gate --only docs-cli --only cli-compat --only docs && cargo test -p toolu-cli --test contract && cargo xtask check-markdown-cli",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S4",
      "S5"
    ],
    "paths": [
      "crates/epic-orchestrator/src/status_schema.rs",
      "crates/epic-orchestrator/src/tests/status_schema_test.rs",
      "docs/epic-status-api.md",
      "docs/config.md",
      "docs/cli",
      "crates/cli/tests",
      "plugins/epic-orchestrator/commands/epic.md",
      "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md",
      "plugins/epic-orchestrator/README.md",
      "AGENTS.md",
      "tooling/conventions/markdown-cli.json"
    ],
    "input": "the built toolu binary's help tree and the Rust endpoint table; the drift gate over plugins/epic-orchestrator Markdown naming toolu epic status --fleet",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9",
      "AC-10"
    ],
    "depends_on": [
      "S6"
    ],
    "paths": [
      "crates",
      "docs",
      "plugins/epic-orchestrator",
      "tooling/conventions",
      "AGENTS.md"
    ],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/epic-orchestrator/src/fleet.rs`, `src/http_fleet.rs`, `src/fleet_cli.rs`, `src/tests/{fleet,http_fleet,http_fleet_proxy,fleet_cli}_test.rs`, `src/tests/dashboard_test.rs`, the test-only `src/tests/{peer_fixture,fleet_rig}_test.rs`, and `docs/toolu/evidence/epic-fleet-dashboard.sh` (force-added).
- Modify: `crates/epic-orchestrator/src/{lib,http,http_response,control,socket,query,verbs,dashboard,status_schema}.rs`, the tests constructing `http::Config` (`http_test.rs`, `http_access_test.rs`, `http_metrics_test.rs`, `control_test.rs`, `attention_ops_test.rs`).
- Docs: `AGENTS.md`, `docs/epic-status-api.md`, `docs/config.md`, `docs/cli/**`, `plugins/epic-orchestrator/{README.md,commands/epic.md,skills/epic-orchestrator/SKILL.md}`, the `commands --json` snapshot in `crates/cli/tests`.

## Verification

End to end: two real in-process engines on loopback serve a fleet document listing both machines and their in-flight issues (AC-1); a blackholed peer and a wrong token each resolve within 3 s while local routes answer in under 500 ms (AC-2); old-schema, non-JSON and 404 peers are `version-mismatch` (AC-3); canary peer tokens never appear in any body, CLI output or resource-root file while peers observe them (AC-4); last-seen, the refresher's first round and round coalescing (AC-5); the proxy and its limit (AC-6); `--fleet` (AC-7); headless Chrome rendering of the real page against two `toolu epic serve` processes, a manual macOS run recorded in the PR (AC-8); hostile peer data (AC-10). Each step's check runs through the managed job wrapper (`job.ts -- <command>`). Delivery then runs `plan-ledger run <plan> --verify`, `toolu-review:review`, `verdict status`, a scoped commit and push of `feat/453-fleet-view-one-status-page`, the PR (`Closes Falconiere/toolu#453`, `Part of Falconiere/toolu#402`) and `pr-babysit:babysit`.

The scoped commit force-adds the gitignored `docs/toolu/` files: the brainstorm, spec, this plan, `docs/toolu/evidence/epic-fleet-dashboard.sh` and `epic-fleet-dashboard.html`. The AC-8 Chrome run is recorded in the PR body; S5's check re-runs it on the verifying machine, which needs Chrome.

## Review

Plan review (2026-10-13): two blockers (missing `http::Config` literals in `http_access_test.rs`/`http_metrics_test.rs`; a `git diff` check that fails on its own regeneration) and should-fixes (pinned test names, AC-4 split per step, dashboard test module, docs phrase assertions, guardrail checks per step, evidence script robustness, force-added files). All applied above.
