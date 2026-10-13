# Epic metrics, baseline and efficiency budgets — Plan

**Date:** 2026-10-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-12-epic-metrics-design.md   **Topic:** Journal-derived epic metrics, the derived TypeScript baseline, and CI budgets (#451)

Plan review: rounds 1–2 by an independent reviewer; the final S10 finding was resolved by widening the fixture module in S10's paths (revision 3).

## Evidence and approach

- Recalled: the journal sequence fix (comemory 46a8f612), the status reader
  boundary (7df19504: whitelist public fields, read journal outside the world
  lock) and the waiting-jobs sidecar (297c77cc).
- Inspected: `journal.rs` (`page`, `read_file`), `status_events.rs`,
  `status_api.rs` `route`/`dynamic_route`, `http.rs` `accept_loop` (20 ms
  poll), `socket.rs` `join_threads` (already wakes both listeners),
  `socket_herdr.rs` (`pause` in 1 s sleeps), `source.rs` `read_events`
  (1 s idle), `dispatch.rs` `reply_wait`/`satisfy`/`expire`, `server.rs`
  `tick`/`flush`/`stamp`, `github_engine.rs` `check_one` notes,
  `babysit_next.rs`, `commit_queue.rs` outcomes, `job.rs` admission,
  `launch/session.rs` transcript lookup, `close.rs` `summary_table`,
  `verbs.rs`/`verbs_actions.rs` (verb wiring), the scenario and queue rigs,
  `tests/status_budget.rs` (Linux `VmHWM` harness), and `crates/cli/tests`
  (`jsonschema` dev-dependency, `CARGO_BIN_EXE_toolu`).
- Probe: `/tmp/w451/probe/baseline.ts` reproduced the issue's counts at cutoff
  `2026-10-05T01:30:00Z`; idle release engine measured at 5.2 MB with 874
  switches per 5 s.
- Design: the spec's fold over journal records in `src/metrics/`, new verbs in
  `src/verbs_metrics.rs` (wired as a third verb group after
  `verbs_actions`), four journal additions, the listener and herdr timer fix.

## Workstream summary

runtime usage probe → metrics math and fold → document and budgets →
baseline verb and committed baseline → journal additions → metrics verb and
API → idle timers and their Linux test → scripted-epic budgets → tokens and
summary (with the closing comment) → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "toolu_runtime::process::self_usage: peak RSS in bytes and CPU ms from getrusage(RUSAGE_SELF)",
    "check": "cargo test -p toolu-runtime -- usage",
    "ac_refs": [
      "AC-8"
    ],
    "paths": [
      "crates/core/runtime/src/process.rs",
      "crates/core/runtime/src/process/usage.rs",
      "crates/core/runtime/src/process/tests/usage_test.rs"
    ],
    "input": "the test process itself after allocating and touching 32 MiB: max_rss_bytes grows by at least that much and cpu_ms is non-decreasing across two calls",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Metrics math: linear percentiles, shares, the streaming journal reader and the epic scope",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics::stats metrics::scope journal::tests",
    "ac_refs": [
      "AC-3"
    ],
    "paths": [
      "crates/epic-orchestrator/src/journal.rs",
      "crates/epic-orchestrator/src/tests/journal_test.rs",
      "crates/epic-orchestrator/src/metrics.rs",
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/tests/metrics_test.rs"
    ],
    "input": "percentiles of [1,2,3,4] and of one value; journal files for two days with a partial last line streamed from a since-day; records keyed by issue, by watch key and unkeyed, scoped to one epic",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Fold: lead time, phases (phase, github-babysit-ready, queue-left) and the ready-to-merged split",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics::timeline metrics::ready",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/metrics.rs",
      "crates/epic-orchestrator/src/tests/metrics_test.rs"
    ],
    "input": "planted journals: an issue launched, ready, engine rebase (rebased), waiting, merged with an empty merge-done note, cleanup-done; a second with a handoff and a queue-left; a third still open with a fixed now_ms; a cleanup-done with a failure note",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Fold: prompts, judgments, wake-ups, GitHub, jobs, launches, engine; the document and budget table",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics::counts metrics::document budgets",
    "ac_refs": [
      "AC-3",
      "AC-6"
    ],
    "depends_on": [
      "S3"
    ],
    "paths": [
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/metrics.rs",
      "crates/epic-orchestrator/src/tests/metrics_test.rs"
    ],
    "input": "planted records per definition, including a held babysit prompt licensed earlier, a launch-wait pair (reason then count), a judgment re-journaled with one token, job attempts refused then admitted, two engine lifetimes; regression plants: babysit prompt after keep-going only, rebase prompt without handoff, timeout wait-reply, late merge-intent with no other repo merge, nextAt 360 s out; an empty journal (every budget unmeasured)",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "toolu epic baseline over TypeScript state directories, its fixture test, the committed benchmarks/epic-baseline.json and a test that pins its content; verb wiring extracted out of verbs.rs (296 code lines)",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics::baseline verbs_metrics && cargo test -p toolu-cli --test epic_metrics baseline",
    "ac_refs": [
      "AC-1",
      "AC-2"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/verbs_metrics.rs",
      "crates/epic-orchestrator/src/verbs.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/cli/tests/epic_metrics.rs",
      "fixtures/epic/baseline",
      "benchmarks/epic-baseline.json",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/verbs_metrics",
      "crates/cli/src/commands.schema.json",
      "crates/epic-orchestrator/src/metrics.rs"
    ],
    "input": "fixtures/epic/baseline: two epics, five issues (one outlier epic, one unmerged, one relaunched, one without a status file) with hand-computed expectations; a malformed issue file; then the real ~/.claude/epics with --until 2026-10-05T01:30:00Z --outlier falconiere-toolu-orm-145 diffed against the committed file; a test reading the committed benchmarks/epic-baseline.json asserts schema 1, 83 merged issues, 542.2 agent-hours, 119 rebase rounds, 11 relaunched and the reported section",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Journal additions in new modules (engine_usage.rs, wait_journal.rs): wait-reply, github-check issue/nextAt/next with stamp to the epic, job-wait, engine-usage; server.rs, dispatch.rs and github_engine.rs stay under 300 code lines",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- dispatch github_engine job server socket wait_journal engine_usage",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "crates/epic-orchestrator/src/dispatch.rs",
      "crates/epic-orchestrator/src/socket_loop.rs",
      "crates/epic-orchestrator/src/github_engine.rs",
      "crates/epic-orchestrator/src/babysit_next.rs",
      "crates/epic-orchestrator/src/server.rs",
      "crates/epic-orchestrator/src/job.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/engine_usage.rs",
      "crates/epic-orchestrator/src/wait_journal.rs",
      "crates/epic-orchestrator/src/babysit.rs",
      "crates/epic-orchestrator/src/watch.rs",
      "crates/epic-orchestrator/src/model.rs"
    ],
    "input": "a real engine with a waiter that times out and one answered by a raised judgment; a PR watch checked against the HTTPS GitHub fixture (keep-going and merge-queue); toolu epic job refused by a full job capacity in a temp resource root; engine start, a tick 15 minutes later and a clean stop",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "toolu epic metrics (text beside the baseline, global --json document) and its schema-validated fixture test",
    "check": "cargo test -p toolu-cli --test epic_metrics && cargo test -p toolu-epic-orchestrator -- verbs_metrics",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "S4",
      "S5"
    ],
    "paths": [
      "crates/epic-orchestrator/src/verbs_metrics.rs",
      "crates/epic-orchestrator/src/metrics",
      "crates/cli/tests/epic_metrics.rs",
      "crates/cli/src/commands.schema.json",
      "fixtures/epic/metrics",
      "crates/epic-orchestrator/src/lib.rs",
      "benchmarks/epic-baseline.json",
      "crates/epic-orchestrator/src/metrics.rs"
    ],
    "input": "fixtures/epic/metrics/journal (two epics, every issue closed) with expected.json and per --epic / --since expectations; an empty resource root; a bad --since; the text form reads the baseline embedded by include_str!(\"../../../../benchmarks/epic-baseline.json\") from src/metrics/text.rs",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "GET /api/v1/metrics in a new http_metrics.rs (query parsing, 60 s 16-entry cache) routed from http.rs, and the generated API reference row",
    "check": "cargo test -p toolu-epic-orchestrator -- status_api status_schema http_metrics http::tests",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "S7"
    ],
    "paths": [
      "crates/epic-orchestrator/src/status_api.rs",
      "crates/epic-orchestrator/src/http.rs",
      "crates/epic-orchestrator/src/status_schema.rs",
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/tests",
      "docs/epic-status-api.md",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/http_metrics.rs"
    ],
    "input": "the fixtures/epic/metrics journal copied into a resource root served by serve_with_http; GET with and without epic/since; ?since=bad",
    "model": "inherit"
  },
  {
    "id": "S9",
    "title": "Idle timers: blocking HTTP accept, herdr Interrupt with 30 s idle and pause, and the Linux idle budget test",
    "check": "cargo test -p toolu-epic-orchestrator -- http socket socket_herdr source socket_deadline && cargo test -p toolu-epic-orchestrator --release --test idle_budget",
    "ac_refs": [
      "AC-7"
    ],
    "paths": [
      "crates/epic-orchestrator/src/http.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/socket_herdr.rs",
      "crates/epic-orchestrator/src/source.rs",
      "crates/epic-orchestrator/src/socket_loop.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/tests/idle_budget.rs",
      "crates/epic-orchestrator/src/lib.rs"
    ],
    "input": "a real engine serving HTTP that stops within 1 s; a herdr fixture subscription stopped and resubscribed mid-read; an idle release engine with no herdr socket (Linux: VmHWM and per-task switches over 35 s; macOS: the test file is cfg-gated and the unit tests pin the constants)",
    "model": "inherit",
    "depends_on": [
      "S6"
    ]
  },
  {
    "id": "S10",
    "title": "Scripted-epic budgets: the scenario rig run with a waiting PR, engine rebase, conflict hand-off, merges, close and a real wait; plus the 180 s slot test; github_engine's fixture test module and its FullTick/rest_routes/graphql_body widen to pub(crate), and the metrics scenario has its own multi-issue engine helper",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics_scenario watch",
    "ac_refs": [
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "S4",
      "S6"
    ],
    "paths": [
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/src/watch.rs",
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/tests/github_engine_fixture_test.rs",
      "crates/epic-orchestrator/src/github_engine.rs"
    ],
    "input": "checks driven through the HTTPS GitHub fixture and FullTick as in src/tests/github_engine_fixture_test.rs (engine, rest_routes, graphql_body): a PR waiting through keep-going checks then green (github-babysit-ready, merge-intent in the same tick); the queue rig's journal from three PRs (one rebased by the engine, one handed off on a conflict); a blocking wait answered by epic-complete; Watch slots after fresh, 304, failed and errored checks",
    "model": "inherit"
  },
  {
    "id": "S11",
    "title": "Token rows (Claude Code with subagents, Codex; OpenCode and Cursor unavailable), toolu epic summary, and the summary in the closing comment",
    "check": "cargo test -p toolu-epic-orchestrator -- metrics::tokens metrics::summary close::tests epic_close verbs_actions::settle verbs_metrics",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S7"
    ],
    "paths": [
      "crates/epic-orchestrator/src/metrics",
      "crates/epic-orchestrator/src/verbs_metrics.rs",
      "crates/epic-orchestrator/src/close.rs",
      "crates/epic-orchestrator/src/tests",
      "fixtures/epic/tokens",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/verbs_actions/settle.rs",
      "crates/epic-orchestrator/src/verbs_actions/tests/settle_test.rs",
      "crates/epic-orchestrator/src/epic_close.rs"
    ],
    "input": "fixtures/epic/tokens: a Claude transcript with a streamed duplicate message id and a subagent file, a Codex rollout with two token_count events; issue records for claude, codex (relaunched), opencode and cursor; a graph whose epic closes",
    "model": "inherit"
  },
  {
    "id": "S12",
    "title": "Documentation: regenerate docs/cli (cargo xtask docs-cli) and the commands snapshot (INSTA_UPDATE=always cargo test -p toolu-cli --test contract), then the README metrics section, skill closing step, benchmarks README and AGENTS.md row",
    "check": "cargo xtask docs-cli --check && cargo xtask check-markdown-cli && cargo test -p toolu-cli --test contract",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "S7",
      "S8",
      "S11"
    ],
    "paths": [
      "docs/epic-orchestrator/README.md",
      "docs/cli",
      "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md",
      "benchmarks/README.md",
      "AGENTS.md",
      "crates/cli/tests"
    ],
    "input": "the built toolu binary's --help for epic metrics, summary and baseline",
    "model": "inherit"
  },
  {
    "id": "S13",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "S1",
      "S2",
      "S3",
      "S4",
      "S5",
      "S6",
      "S7",
      "S8",
      "S9",
      "S10",
      "S11",
      "S12"
    ],
    "paths": [
      "crates",
      "docs",
      "fixtures",
      "benchmarks",
      "plugins",
      "AGENTS.md",
      "tooling"
    ],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- New: `crates/core/runtime/src/process/usage.rs`;
  `crates/epic-orchestrator/src/metrics.rs` and `src/metrics/{stats,scope,timeline,ready,counts,document,budgets,baseline,tokens,summary,text}.rs`
  with `src/metrics/tests/*_test.rs`; `src/verbs_metrics.rs` (+ `src/verbs_metrics/`); `src/engine_usage.rs`, `src/wait_journal.rs`, `src/http_metrics.rs` with tests;
  `crates/epic-orchestrator/tests/idle_budget.rs`;
  `crates/cli/tests/{epic_baseline,epic_metrics}.rs`;
  `fixtures/epic/{baseline,metrics,tokens}/`;
  `docs/epic-orchestrator/metrics.schema.json`; `benchmarks/epic-baseline.json`.
- Modified: `journal.rs`, `dispatch.rs`, `socket_loop.rs`, `github_engine.rs`,
  `babysit_next.rs`, `server.rs`, `job.rs`, `socket.rs`, `http.rs`,
  `socket_herdr.rs`, `source.rs`, `status_api.rs`, `status_schema.rs`,
  `close.rs`, `epic_close.rs`, `verbs_actions/settle.rs`, `verbs.rs` (one wiring line), `lib.rs`, `watch` tests; docs listed in S12.

## Verification

- End to end: `toolu epic baseline` against the real state directories equals
  the committed file (AC-1, recorded in the PR); the CLI and HTTP documents on
  the fixture journal equal the hand-computed expectation and validate against
  the schema (AC-3, AC-4); the scripted epic passes its budgets and each
  planted regression fails its budget (AC-5, AC-6); the Linux idle test runs
  in the `rust` CI job (AC-7) and its figures go into the PR body.
- Boundaries: empty journal, journal without the new records (budgets
  `unmeasured`), `--since` cutting intervals, malformed baseline input (exit 1),
  bad queries (exit 2 / HTTP 400), missing transcripts (`unavailable`).
- Every file stays under 300 code lines and every function under 50; `cargo
  xtask gate` is the final word (S13).
- Documentation synchronized in S12 and checked by `docs-cli`,
  `check-markdown-cli` and the contract snapshot.

## Deviations

- S2–S5 and S7 landed together: workspace lints deny dead code, so the fold
  compiles only once a verb uses it. Each step's check still runs on its own.
- The metrics and baseline schemas are `$defs/epicMetrics`,
  `$defs/epicPercentiles`, `$defs/epicBaseline` and `$defs/epicBaselineGroup`
  in `crates/cli/src/commands.schema.json` (served by `toolu commands
  --schema`), following `epicQueue` and `epicStatus`, instead of a separate
  `docs/epic-orchestrator/metrics.schema.json`. The baseline CLI test lives in
  `crates/cli/tests/epic_metrics.rs` beside the metrics test.
- `relaunches` counts successful `launch-done` records (empty note) beyond the
  first, not `launch-intent` records: a launch that waited for capacity
  journals a second intent without being a relaunch.
- `prompts.decided_checks` (checks that carried a babysit `next`) gates the
  `prompts-while-waiting` budget, so a journal from before this change reports
  it `unmeasured` instead of failing it.
- A bad `--since` exits with `Exit::Usage` (64, sysexits `EX_USAGE`), the
  repository's usage code, not 2.
- Phase shares keep four decimals by truncation.
- `wait-reply`, `job-wait` and `engine-usage` are appended straight to the
  journal (as the existing `spool` record is), not through the outbox: the
  reply is already sent, and a flush in the middle of a dispatch could apply
  unrelated queued steps.
