# Dashboard control API — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-dashboard-control-api-design.md   **Topic:** Token-gated `POST /api/v1/actions/{op}` (#650)

## Evidence and approach

Inspected these files:
- `http.rs`, `http_request.rs`, `http_access.rs`, `http_response.rs`;
- `socket.rs`, `socket_loop.rs` (`Msg::Line`), `dispatch.rs`, `dispatch_ops.rs`, `attention_ops.rs`, `holds.rs`, `server.rs`;
- `verbs.rs` (`token new`), `query.rs` (CLI `ask`), `notify_notice::dashboard`;
- `toolu_runtime::config::secrets` (`load`, `path`, `rotate_status_token`);
- the tests `src/tests/http_test.rs` (the `live_fixture` real engine) and `crates/cli/tests/epic_token.rs`.

Recalled comemory `38cad20a`, which says control is local-engine only and needs a token on every mutation.

The approach follows the spec:
- HTTP threads forward an allow-listed op as `Msg::Line` to the state loop with a deadline.
- The engine gains `by`, journaling and an uncertain-answer reply that the CLI shares.
- The secrets reload in the listener is driven by mtime.
- `http.json` records the bound address, and `token url` reads it.

## Workstream summary

request body reader → engine `by`/journal/uncertain → live secrets → action route + gates → `http.json` + `token url` → docs and CLI reference → full gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "request parser reads a bounded POST body and captures Origin and Content-Type", "check": "cargo test -p toolu-epic-orchestrator http_request", "ac_refs": ["AC-2"], "paths": ["crates/epic-orchestrator/src/http_request.rs", "crates/epic-orchestrator/src/tests/http_request_test.rs"], "input": "raw HTTP/1.1 bytes over a real TcpStream: POST with a JSON body, POST without Content-Length, POST over 16 KiB, GET with a body, Transfer-Encoding", "model": "inherit"},
  {"id": "s2", "title": "engine pause/resume journal with by, release by, uncertain answer reply", "check": "cargo test -p toolu-epic-orchestrator dispatch && cargo test -p toolu-epic-orchestrator holds && cargo test -p toolu-epic-orchestrator attention_ops && cargo test -p toolu-epic-orchestrator logic", "ac_refs": ["AC-1", "AC-3", "AC-4"], "paths": ["crates/epic-orchestrator/src/server.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/dispatch_ops.rs", "crates/epic-orchestrator/src/holds.rs", "crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs", "crates/epic-orchestrator/src/tests/dispatch_ops_test.rs", "crates/epic-orchestrator/src/tests/holds_test.rs", "crates/epic-orchestrator/src/tests/attention_ops_test.rs", "crates/epic-orchestrator/src/tests/attention_redirect_test.rs", "crates/epic-orchestrator/src/tests/logic_test.rs"], "input": "Engine::open on a temp resource root with a registered epic; scripts dir whose prompt prints {\"outcome\":\"uncertain\"}; real herdr error JSON with codes timeout and agent_prompt_stalled", "model": "inherit"},
  {"id": "s3", "title": "listener re-reads a rotated secrets.json at most once a second, keeping the last good on error", "check": "cargo test -p toolu-epic-orchestrator http_secrets", "ac_refs": ["AC-7"], "paths": ["crates/epic-orchestrator/src/http_secrets.rs", "crates/epic-orchestrator/src/tests/http_secrets_test.rs", "crates/epic-orchestrator/src/http.rs"], "input": "a real 0600 secrets.json under a temp HOME, rotated with secrets::rotate_status_token, then made unsafe (0644)", "model": "inherit"},
  {"id": "s4", "title": "POST /api/v1/actions/{op} gates, validates and forwards to the state loop with a deadline", "check": "cargo test -p toolu-epic-orchestrator http", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-8", "AC-9"], "depends_on": ["s1", "s2", "s3"], "paths": ["crates/epic-orchestrator/src/http.rs", "crates/epic-orchestrator/src/http_action.rs", "crates/epic-orchestrator/src/http_response.rs", "crates/epic-orchestrator/src/socket.rs", "crates/epic-orchestrator/src/control.rs", "crates/epic-orchestrator/src/tests/http_action_test.rs", "crates/epic-orchestrator/src/tests/http_test.rs", "crates/epic-orchestrator/src/tests/fleet_rig_test.rs", "crates/epic-orchestrator/src/tests/http_access_test.rs", "crates/epic-orchestrator/src/tests/http_metrics_test.rs"], "input": "real engine via serve_with_http on 127.0.0.1 with token CANARY-TOKEN-650, a registered epic with a held issue and a live worker, scripts dir with prompt; raw TCP requests per gate", "model": "inherit"},
  {"id": "s5", "title": "engine writes http.json; toolu epic token url prints the fragment URL or names why not", "check": "cargo test -p toolu-epic-orchestrator socket && cargo build -p toolu-cli && cargo test -p toolu-cli --test epic_token_url", "ac_refs": ["AC-5", "AC-6"], "depends_on": ["s4"], "paths": ["crates/epic-orchestrator/src/paths.rs", "crates/epic-orchestrator/src/socket.rs", "crates/epic-orchestrator/src/verbs.rs", "crates/epic-orchestrator/src/verbs_token.rs", "crates/cli/tests/epic_token_url.rs"], "input": "the built toolu binary with temp HOME/TOOLU_CONFIG_DIR/TOOLU_RESOURCE_HOME, TOOLU_EPIC_HERDR=off, a live `epic engine --http 127.0.0.1:0` process; no engine; no token", "model": "inherit"},
  {"id": "s6", "title": "status API docs, orchestrator README and generated CLI reference describe actions and token url", "check": "cargo xtask docs-cli && git diff --exit-code docs/cli && cargo xtask check-markdown-cli && cargo xtask check-cli-compat", "ac_refs": ["AC-9"], "depends_on": ["s5"], "paths": ["docs/epic-status-api.md", "docs/epic-orchestrator/README.md", "docs/cli"], "input": "the real docs and the clap tree", "model": "inherit"},
  {"id": "s7", "title": "full local gate", "check": "DOCKER_CONTEXT=colima-toolu-gate cargo xtask deploy-gate", "ac_refs": ["AC-9"], "depends_on": ["s6"], "input": "whole workspace", "model": "inherit"}
]
```

## Critical files

- **New:** `crates/epic-orchestrator/src/http_action.rs`, `http_secrets.rs` and `verbs_token.rs`, with tests `src/tests/http_action_test.rs` and `src/tests/http_secrets_test.rs`, plus `crates/cli/tests/epic_token_url.rs`.
- **Modified:** `http.rs`, `http_request.rs`, `http_response.rs`, `socket.rs`, `control.rs`, `server.rs`, `dispatch.rs`, `dispatch_ops.rs`, `holds.rs`, `attention_ops.rs`, `paths.rs` and `verbs.rs`, the existing tests that construct `http::Config`, `docs/epic-status-api.md`, `docs/epic-orchestrator/README.md` and `docs/cli/*`.

## Verification

Real engines serve real TCP requests for every gate and op.
- The journal and the SSE stream show `action pause` with `by=web`, and the socket op shows `by=user`.
- The refusals leave the journal line count unchanged.
- The token canary is absent from bodies, the journal, the resource root and the engine's output.
- `token url` runs against the real binary.
- The AC-10 probe against real herdr is recorded in `docs/toolu/evidence/`.
- The docs are regenerated and checked. Finally, `cargo xtask gate` runs.

## Plan review (2026-10-27)

- **Checks:** every step's check is runnable and names its real input.
  - s4's check `cargo test -p toolu-epic-orchestrator http` covers the existing HTTP suites (AC-9) as well as the new action tests.
  - s6 compares the regenerated CLI docs.
- **Ordering:** s4 needs the parser (s1), the engine `by` and journaling (s2) and live secrets (s3). s5 needs the `Config` changes from s4.
- **Docs:** s6 owns the user-facing documentation.
- **Size:** `http.rs` already holds 326 physical lines, so the action route lives in the new `http_action.rs` and the reload in `http_secrets.rs`. That keeps each file under 300 code lines.
- **Status:** Approved.

## Deviations

- **`http_test`'s boundary test.** It now posts with `Content-Length: 0` to keep asserting `405` and `Allow: GET, HEAD`, because #650 makes a `POST` without a length `400`. The stream replay test moved unchanged to `http_read_test.rs` with the read routes (`http_read.rs`), which split out to keep `http.rs` under 300 code lines.
- **Pause journaling.** It lives in `dispatch.rs` rather than in `Engine::pause` and `resume`, which keeps `server.rs` under 300 code lines. Those methods keep their signatures.
- **Directed answers.** An answer with no open attention item now also journals `judgment directed` with `by=<who>`, so every web answer records `by=web`, as the issue's scenario requires.
- **AC-10 probe.** A built engine on a temporary resource home answered this worker's own herdr agent (`falconiere-toolu-650`) through `POST /api/v1/actions/answer`. It replied `200 {"ok":true}`, the text arrived in the agent session, and the journal holds the `prompt answer` row. The token was absent from the engine output, the journal and `http.json`.
- **Step s7.** It runs the full `cargo xtask gate` in the clean container (`cargo xtask deploy-gate`) instead of on the host. On this Mac the host gate fails for environmental reasons (the Homebrew toolu, the live epic engine), as `.toolu` gate notes record. The plan ledger already holds the expensive-job lease, so it is not wrapped in `toolu epic job`.
- **Code review (`/code-review xhigh --fix`).** It found 15 issues; all are fixed but one:
  - the action deadline is now 35 s, because herdr's prompt timeout is 30 s;
  - a token rotation ends SSE streams opened with the old token;
  - the secrets reload compares file content, not just mtime;
  - uncertain answers journal their text;
  - an IPv4 loopback client is accepted on a `::` bind;
  - `token url --json` prints JSON;
  - a stale `http.json` is removed;
  - directed answers are left out of the attention metrics.

  The one left open: `GET /` needing the token on a non-loopback bind is pre-existing listener policy, and changing it is a decision for the UI issue #656.
