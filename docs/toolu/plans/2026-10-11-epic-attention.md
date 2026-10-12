# Epic attention queue and notifications — Plan

**Date:** 2026-10-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-11-epic-attention-design.md   **Topic:** #450, attention queue, answers, reminders, herdr and outgoing notifications

## Evidence and approach

Inspected `crates/epic-orchestrator`:

- `model::Attention` and `logic::push_attention` raise items.
- `schedule::{ack, take_judgment, on_tick}`, `dispatch::{reply_answer, reply_ack, satisfy}`, `status::{document, wait_body}`, `status_api::attention`, `status_epics` and `recover::observe` read or replay them.
- `server::Engine::{apply, send_prompt}` applies steps and sends worker prompts.
- `socket::{ServeOptions, state_loop}`, `socket_deadline::wake_after` and `control::http_config` run the loop and load its config.
- `limit::{on_status, mark_blocked}`, `source_apply::status_event`, `herdr::{Call, Client}` and `herdr_argv` handle herdr.

Also inspected `toolu_runtime::config::{epic, secrets}` (#463) and `toolu_http::{Client, Request, check_url}`, plus `toolu-http-test-support::Fixture`, the real loopback HTTPS fixture already used by `github_probe_test.rs`. Memory recalled #449's rule: HTTP documents are allow-listed and carry no pane text.

The approach follows the approved spec:

- A new `attention` module owns the queue's states, dedup, close, ack, answer, reminders and rebuild.
- `notify` and `notify_notice` run the notifier thread, build the payload and POST with retries.
- `Step::Notify` connects the state machine to the thread.
- The config gains `epic.attention.herdr` and `epic.attention.remind_after_minutes`.
- Existing files stay under 300 code lines by moving queue logic out of `logic.rs`, `schedule.rs` and `dispatch.rs`.

## Workstream summary

Config keys, then the queue in the state machine, then the notifier thread and herdr call. Then the engine wiring (answer, ack, blocked, wake, serve options) and the documents. Then docs, schemas and generated references, and last the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "config",
    "title": "Parse epic.attention.herdr and epic.attention.remind_after_minutes with defaults and path-only errors",
    "check": "cargo test -p toolu-runtime --lib -- config::epic::",
    "ac_refs": [
      "AC-9"
    ],
    "paths": [
      "crates/core/runtime/src/config/epic.rs",
      "crates/core/runtime/src/config/tests/epic_test.rs"
    ],
    "input": "LoadedConfig::from_data with defaults, valid values, 0, 10081, a string, and a notify_url key"
  },
  {
    "id": "queue",
    "title": "Attention states, dedup, close on report with waited_ms, ack, answer marking, hourly reminders and journal rebuild",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- reminder_ closed_records_ attention_rebuild ack_silences_ attention:: schedule:: logic:: recover:: status::",
    "ac_refs": [
      "AC-3",
      "AC-5",
      "AC-7",
      "AC-11"
    ],
    "depends_on": [
      "config"
    ],
    "paths": [
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/model.rs",
      "crates/epic-orchestrator/src/attention.rs",
      "crates/epic-orchestrator/src/logic.rs",
      "crates/epic-orchestrator/src/schedule.rs",
      "crates/epic-orchestrator/src/recover.rs",
      "crates/epic-orchestrator/src/snapshot.rs",
      "crates/epic-orchestrator/src/status.rs",
      "crates/epic-orchestrator/src/note.rs",
      "crates/epic-orchestrator/src/tests/attention_test.rs",
      "crates/epic-orchestrator/src/tests/schedule_test.rs",
      "crates/epic-orchestrator/src/tests/logic_test.rs",
      "crates/epic-orchestrator/src/tests/recover_test.rs",
      "crates/epic-orchestrator/src/tests/status_test.rs",
      "crates/epic-orchestrator/src/tests/note_test.rs"
    ],
    "input": "A real World driven through reports and ticks with a moving now_ms, and a real journal directory replayed by snapshot::load"
  },
  {
    "id": "notify",
    "title": "Notifier thread: bounded queue, herdr notification.show, POST to the secret URL with 5 attempts and backoff, redacted payload",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- notify_retries herdr_notification notify:: notify_notice:: herdr::",
    "ac_refs": [
      "AC-4",
      "AC-8"
    ],
    "depends_on": [
      "queue"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/notify.rs",
      "crates/epic-orchestrator/src/notify_notice.rs",
      "crates/epic-orchestrator/src/herdr.rs",
      "crates/epic-orchestrator/src/herdr_argv.rs",
      "crates/epic-orchestrator/src/tests/notify_test.rs",
      "crates/epic-orchestrator/src/tests/notify_notice_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_argv_test.rs"
    ],
    "input": "toolu-http-test-support HTTPS fixture replying 500, 500, 200 and dropping connections; a UnixListener speaking herdr protocol 22; TOOLU_EPIC_NOTIFY_URL and a canary status token in an explicit Env"
  },
  {
    "id": "engine",
    "title": "Wire Step::Notify, answer through the worker prompt with --by, ack by key or kind, blocked pending tail, reminder wake and serve options",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- attention_reaches_every_channel answer_ blocked_pending && cargo test -p toolu-epic-orchestrator",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "queue",
      "notify"
    ],
    "paths": [
      "crates/epic-orchestrator/src/server.rs",
      "crates/epic-orchestrator/src/attention_ops.rs",
      "crates/epic-orchestrator/src/tests/attention_ops_test.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/socket_deadline.rs",
      "crates/epic-orchestrator/src/control.rs",
      "crates/epic-orchestrator/src/dispatch.rs",
      "crates/epic-orchestrator/src/query.rs",
      "crates/epic-orchestrator/src/verbs.rs",
      "crates/epic-orchestrator/src/limit.rs",
      "crates/epic-orchestrator/src/source_apply.rs",
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/status_epics.rs",
      "crates/epic-orchestrator/src/dashboard.rs",
      "crates/epic-orchestrator/src/tests/dispatch_test.rs",
      "crates/epic-orchestrator/src/tests/socket_test.rs",
      "crates/epic-orchestrator/src/tests/server_test.rs",
      "crates/epic-orchestrator/src/tests/limit_test.rs",
      "crates/epic-orchestrator/src/tests/control_test.rs",
      "crates/epic-orchestrator/src/tests/query_test.rs",
      "crates/epic-orchestrator/src/tests/verbs_test.rs",
      "crates/epic-orchestrator/src/tests/status_api_test.rs",
      "crates/epic-orchestrator/src/tests/http_test.rs"
    ],
    "input": "A real engine thread on a temp resource root with HTTP on 127.0.0.1:0, an issues/a.json with a url, a TOOLU_EPIC_SCRIPTS prompt script that appends its stdin to a file, and the HTTPS fixture as notify URL"
  },
  {
    "id": "docs",
    "title": "Engine attention table in the skill, README, config docs, status API doc, CLI schema and generated CLI reference",
    "check": "cargo test -p toolu-epic-orchestrator --lib status_schema && cargo xtask gate --only docs-cli && cargo xtask check-markdown-cli && cargo test -p toolu-cli --test contract",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "engine"
    ],
    "paths": [
      "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md",
      "plugins/epic-orchestrator/README.md",
      "docs/config.md",
      "docs/epic-status-api.md",
      "docs/cli/epic.md",
      "docs/cli/commands.json",
      "crates/epic-orchestrator/src/status_schema.rs",
      "crates/epic-orchestrator/src/tests/status_schema_test.rs",
      "crates/cli/src/commands.schema.json",
      "crates/cli/tests"
    ],
    "input": "The edited Markdown and the real toolu epic command tree"
  },
  {
    "id": "gate",
    "title": "Full Rust quality gate on the branch",
    "check": "cargo xtask gate --base origin/main",
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
      "AC-10",
      "AC-11"
    ],
    "depends_on": [
      "config",
      "queue",
      "notify",
      "engine",
      "docs"
    ],
    "paths": [
      "crates",
      "Cargo.toml",
      "Cargo.lock",
      "docs",
      "plugins/epic-orchestrator"
    ],
    "input": "The whole branch"
  }
]
```

## Critical files

- Create: `crates/epic-orchestrator/src/attention.rs`, `notify.rs`, `notify_notice.rs` and their `src/tests/*_test.rs`.
- Modify: `crates/epic-orchestrator/src/{model,logic,schedule,recover,status,status_api,status_epics,dispatch,query,verbs,server,socket,socket_deadline,control,limit,source_apply,herdr,herdr_argv,dashboard,status_schema,lib,note}.rs`, `crates/epic-orchestrator/Cargo.toml`, and `crates/core/runtime/src/config/epic.rs`.
- Docs: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`, `plugins/epic-orchestrator/README.md`, `docs/config.md`, `docs/epic-status-api.md`, `docs/cli/`, `crates/cli/src/commands.schema.json`.

## Verification

- End to end (AC-1, AC-2): a real engine, a report over the Unix socket, the real HTTP listener, the real HTTPS fixture and the prompt script. Check the `wait` document, `/api/v1/attention`, one POST and its payload, and the answer delivery. Then a closing report and its journal record.
- Failure and boundary checks:
  - AC-4: a URL failing with 500s, a URL dropping connections, and engine responsiveness during the retries.
  - AC-2: no live item, and an undelivered prompt.
  - AC-6: no pane text in the HTTP documents.
  - AC-7: legacy records without a token.
  - AC-9: config values out of range.
- Documentation is kept in sync by the `docs` step's generated-reference and Markdown–CLI checks. The `gate` step runs coverage, guardrails (file, function and `impl` lengths, co-located tests), clippy, docs-cli and cli-compat.
- Delivery follows this order:
  1. A scoped `feat(epic):` commit after each green step (`wip:` subjects are allowed).
  2. `plan-ledger.js run <plan> --verify` over the whole branch diff.
  3. `toolu-review:review` with the version 2 state, then `verdict.js status` reporting `overall: ready`.
  4. A rebase on `origin/main` if it moved, with the affected checks re-run.
  5. Push, and a PR to `main` whose body starts with `Closes Falconiere/toolu#450` and `Part of Falconiere/toolu#402`.
  6. `report pr-open`, `report babysit`, then the `pr-babysit:babysit` handoff.

## Deviations

- `engine`: the `ack` and `answer` socket operations live in a new `attention_ops.rs` (with `tests/attention_ops_test.rs`, which holds the `answer_` tests), because moving them into `dispatch.rs` or `server.rs` put either file over the 300-code-line limit.
- `answer --by` defaults to the `--host` name, else `user`. Environment detection always names a host, so it cannot tell a person at a terminal from an agent.
- Pre-push review: a stall that returns after `ack`, or a new `blocked` spell, merged silently into the seen item. These edge-triggered raises now go through `attention::reraise`, which reopens an item a waiter saw, or one acknowledged or answered, journals `judgment reopened` and announces it again. Level-triggered repeats (babysit every tick) keep plain dedup. Jev chose this over reopening on every raise (0.96).
- Pre-push review: `answer` propagates a journal failure after delivery as an engine error, as `ack` does.

## Plan review

Round 1. AC references cover every spec criterion with no dangling id, and the dependencies are acyclic. Jev scored step/AC alignment 1.9 out of 3: every criterion is observable, but some checks were broad module filters. Fixes made:

- Checks: 🟡 should-fix: they now name the spec's tests (`reminder_`, `closed_records_`, `attention_rebuild`, `ack_silences_`, `notify_retries`, `herdr_notification`, `attention_reaches_every_channel`, `answer_`, `blocked_pending`). The engine step also runs the whole crate.
- `engine`: 🟡 should-fix: AC-5's real-engine journal evidence is now mapped there as well.
- `docs`: 🔵 consider: the schema check is in the `contract` target, not a separate `schema` target. Fixed.
- Verification: 🟡 should-fix: the delivery order (ledger verify, review, verdict, push, PR, babysit) is now stated.

**Status:** Approved.
