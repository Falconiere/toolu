# Pause and resume an agent host — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-pause-resume-host-design.md   **Topic:** Pause/resume a worker host from the CLI and the dashboard (#653)

## Evidence and approach

Inspected: `limit.rs` (two cooldown stores), `route/capacity.rs` + `route/pool.rs` + `route/assign.rs` (Capacity, pick_host, keep), `dispatch.rs` (op match, `journal_pause`), `http_action.rs` (#650 gate and forwarding), `status_resources.rs` (`/api/v1/hosts`), `status_schema.rs` (generated docs), `verbs.rs` + `verbs_token.rs` (verb-group pattern), `query.rs` (`ask_checked`), `launch.rs` (`resolve_host`, manual launch), `effects_launch.rs` + `commit.rs` + `refill.rs` (wait outcome, `note_wait`, `mark_relaunch`), `hosts.rs` (`host_kind` and its valid-host message), and the `http_action` real-engine test rig. The issue's review additions settle the storage shape, the two-store resume, the waiting relaunch and the manual-launch refusal.

Approach: a new `host_pause` module owns the shared-store pause (read, pause, resume clearing both cooldown stores); `Capacity` learns a distinct `paused` set; dispatch gains `host-pause`/`host-resume` with journaling; `http_action` forwards the same ops; a `hosts` CLI verb group and the launch guards complete the surface.

## Workstream summary

store (`host_pause`) → routing + status → socket ops + journal → HTTP actions → CLI verbs + launch guards → docs/CLI reference → full gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "host_pause module: paused_hosts in the shared store; resume clears shared and per-epic cooldowns", "check": "cargo test -p toolu-epic-orchestrator host_pause", "ac_refs": ["AC-2", "AC-3"], "paths": ["crates/epic-orchestrator/src/host_pause.rs", "crates/epic-orchestrator/src/tests/host_pause_test.rs", "crates/epic-orchestrator/src/lib.rs"], "input": "a temporary TOOLU_RESOURCE_HOME with a real state.json written through update_resources; hosts.json files in two fake epic state dirs; a cooldown written by limit.rs's write path shape", "model": "inherit"},
  {"id": "s2", "title": "Capacity gains paused, routing skips paused hosts, /api/v1/hosts shows paused/paused_by/paused_at", "check": "cargo test -p toolu-epic-orchestrator capacity && cargo test -p toolu-epic-orchestrator assign && cargo test -p toolu-epic-orchestrator status_resources && cargo test -p toolu-epic-orchestrator status_schema", "ac_refs": ["AC-1", "AC-2", "AC-3"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/route/capacity.rs", "crates/epic-orchestrator/src/route/assign.rs", "crates/epic-orchestrator/src/route/tests/capacity_test.rs", "crates/epic-orchestrator/src/route/tests/assign_test.rs", "crates/epic-orchestrator/src/status_resources.rs", "crates/epic-orchestrator/src/tests/status_resources_test.rs", "crates/epic-orchestrator/src/status_schema.rs"], "input": "scripted graph (support::graph_248) in a temp state dir, shared store with paused_hosts and cooldowns, Snapshot::open on a temp resource root", "model": "inherit"},
  {"id": "s3", "title": "control-socket host-pause/host-resume ops validate, apply and journal with by", "check": "cargo test -p toolu-epic-orchestrator dispatch", "ac_refs": ["AC-4", "AC-5"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs"], "input": "Engine::open on a temp resource root with a registered epic in registry.json; socket requests with by absent, user and web; an unknown host", "model": "inherit"},
  {"id": "s4", "title": "POST /api/v1/actions/host-pause and host-resume through the #650 gate", "check": "cargo test -p toolu-epic-orchestrator http_action", "ac_refs": ["AC-4", "AC-5"], "depends_on": ["s3"], "paths": ["crates/epic-orchestrator/src/http_action.rs", "crates/epic-orchestrator/src/tests/http_action_test.rs", "crates/epic-orchestrator/src/tests/http_action_ops_test.rs"], "input": "real engine via serve_with_http on loopback with a status token, raw TCP POSTs: valid host, alias, unknown host, missing host, wrong token, foreign Origin", "model": "inherit"},
  {"id": "s5", "title": "toolu epic hosts pause|resume CLI, manual-launch refusal with --force, waiting relaunch on a paused host", "check": "cargo test -p toolu-epic-orchestrator verbs && cargo test -p toolu-epic-orchestrator launch && cargo test -p toolu-epic-orchestrator effects_launch", "ac_refs": ["AC-5", "AC-7", "AC-8"], "depends_on": ["s2", "s3"], "paths": ["crates/epic-orchestrator/src/verbs.rs", "crates/epic-orchestrator/src/verbs_hosts.rs", "crates/epic-orchestrator/src/tests/verbs_hosts_test.rs", "crates/epic-orchestrator/src/launch.rs", "crates/epic-orchestrator/src/effects_launch.rs"], "input": "the clap tree for hosts pause/resume with valid, alias and unknown hosts; a temp resource home with codex paused, a ready issue launched --kind codex with and without --force (dry-run); a running worker record on codex whose relaunch is armed", "model": "inherit"},
  {"id": "s6", "title": "CLI reference, status API docs and orchestrator README describe host pause/resume", "check": "cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo xtask check-cli-compat", "ac_refs": ["AC-6"], "depends_on": ["s5"], "paths": ["docs/cli", "docs/epic-status-api.md", "docs/epic-orchestrator/README.md", "crates/cli/tests/fixtures/commands_json.snap"], "input": "the real clap tree and docs; status docs regenerated with TOOL_U_UPDATE_DOCS=1 cargo test status_api_doc", "model": "inherit"},
  {"id": "s7", "title": "full quality gate and CLI contract checks", "check": "toolu epic job -- cargo xtask gate", "ac_refs": ["AC-6"], "depends_on": ["s6"], "input": "whole workspace", "model": "inherit"}
]
```

## Critical files

- **New:** `crates/epic-orchestrator/src/host_pause.rs` (+ `src/tests/host_pause_test.rs`), `crates/epic-orchestrator/src/verbs_hosts.rs` (+ `src/tests/verbs_hosts_test.rs`).
- **Modified:** `crates/epic-orchestrator/src/lib.rs`, `route/capacity.rs`, `route/assign.rs`, `status_resources.rs`, `status_schema.rs`, `dispatch.rs`, `http_action.rs`, `verbs.rs`, `launch.rs`, `effects_launch.rs`, their sibling tests, `docs/epic-status-api.md`, `docs/epic-orchestrator/README.md`, `docs/cli/*`, `crates/cli/tests/fixtures/commands_json.snap`.

## Verification

Real engines and real stores throughout: the cooldown is written by feeding a limit line through `limit.rs`'s path in the engine rig; routing uses the scripted graph; HTTP uses raw TCP against `serve_with_http`; the CLI verbs are exercised against the built binary or the clap tree with a temp resource home. Red first for each behavior step, then green. Docs are regenerated, never hand-edited where a generator owns them. Finally `toolu epic job -- cargo xtask gate` plus `check-cli-compat` and `check-markdown-cli`.

## Plan review (2026-10-27)

- **AC coverage:** AC-1…AC-8 each map to at least one step (`ac_refs` cross-checked against the spec; no dangling refs, dependency order is acyclic and topologically valid).
- **Runnable evidence:** every step names a cargo check and a real input, including boundary cases (unknown host, alias, missing host, wrong token, foreign Origin, `--force`).
- **Docs:** s6 owns the CLI reference, status API docs and README, with `docs-cli`, `check-markdown-cli` and `check-cli-compat` as its check.
- **Delivery:** s7 runs the full gate under `toolu epic job`; execution then follows the delivery preflight (ledger `--verify`, review, verdict, push, PR).
- **Status:** Approved.
