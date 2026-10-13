# Epic metrics, baseline and efficiency budgets — Design

**Date:** 2026-10-12   **Status:** Draft   **Author:** Claude (epic worker, #451)   **Topic:** Measure epic runs from the engine journal, against a derived TypeScript baseline, under CI budgets

Brainstorm: `docs/toolu/brainstorms/2026-10-12-epic-metrics.md`.

## Problem

The epic engine (#434, #448, #449) was built to remove waits and LLM wake-ups
measured by hand from the TypeScript watcher's state files. Nothing turns its
journal into numbers yet, so nobody can tell whether a run met the targets,
and nothing stops a change that quietly brings back a timer that wakes an LLM.
The baseline numbers in #451 were computed once, by hand, with no committed
method.

## Non-Goals

1. No change to what the engine decides or does (gate, queue, merge, launch).
2. No fleet view across machines, no write API, no new HTTP surface beyond
   `GET /api/v1/metrics`.
3. No estimated value presented as a measurement. Anything not recorded is
   `null` with a reason.
4. No OpenCode or Cursor token reading (OpenCode keeps usage in SQLite, Cursor
   in no readable store); both are reported `unavailable`.
5. Attaching the first real engine-run report to #451 is an operational step
   after merge, not part of this change (see Open Questions).

## Architecture

All code lives in `crates/epic-orchestrator` except one usage probe in
`toolu-runtime`.

- **`metrics/` module** (new): a pure computation over journal `Record`s
  (`journal::page` already reads every retained file). Submodules:
  `timeline` (per-issue ordered records), `stats` (linear-interpolated
  percentiles, shares), `ready` (first `ready` to merged, split), `document`
  (the JSON document), `budgets` (the budget table), `baseline` (TypeScript
  state directories), `tokens` (session-log usage), `summary` (Markdown).
  Durations are milliseconds; the same `stats` code serves the journal and the
  baseline.
- **Journal additions** (closed record shape unchanged; new names only):
  1. `action`/`wait-reply`, written when `toolu epic wait` returns to its
     caller: note `judgment:<kind>` or `timeout`. A wake-up of the
     orchestrator LLM is one such reply; a `timeout` reply is judgment-free.
  2. The `github-check` note gains `"next"`: the babysit decision of that
     check (`keep-going`, `merge-queue`, `prompt`, `escalate`, `attention`),
     or absent when no babysit ran. No extra line per check.
  3. `admission`/`job-wait`, written when `toolu epic job` is refused: note
     `capacity` or `pressure` (the refusal starting `resource hold:`).
  4. `action`/`engine-usage`, written by the engine at most every 15 minutes
     from its 30 s tick and once at shutdown: note
     `{"max_rss_bytes","cpu_ms","voluntary_switches"}` from
     `getrusage(RUSAGE_SELF)` (new `toolu_runtime::process::self_usage`).
- **Idle timers.** The HTTP listener stops polling a non-blocking `accept`
  every 20 ms; it blocks in `accept`, and `join_threads` already connects to
  wake it, after which it checks the stop flag before serving. The herdr
  subscription keeps its blocking read with a 30 s idle timeout instead of
  1 s; stop and resubscribe shut the stream down through a shared handle, so
  neither waits for the timeout. With no client, no epic and no herdr, an
  idle engine then wakes only on its 30 s tick.
- **Surfaces.** `toolu epic metrics` and `/api/v1/metrics` serve the same
  document (`metrics::document`), read straight from the journal (no engine
  needed for the CLI). `toolu epic summary` renders it beside the baseline,
  which is embedded at build time with
  `include_str!("../../../benchmarks/epic-baseline.json")`, plus budgets and
  token use. `close::summary_table` appends the summary to the epic's closing
  comment. `toolu epic baseline` derives `benchmarks/epic-baseline.json`.
- **Budgets in CI.** A scripted-epic test computes the document from the
  scenario rig's real journal and asserts every deterministic budget; a
  regression test plants the failure each budget names. A Linux integration
  test (`tests/idle_budget.rs`, like `tests/status_budget.rs`) starts a
  release `toolu epic serve --http`, idles, and asserts `VmHWM` ≤ 10 MiB and
  at most 2 voluntary context switches per thread over a 3 s window (one 30 s
  tick fits in no 3 s window more than once).

Decisive trade-off: one metrics implementation in the engine crate (Jev 0.59)
over an `xtask` copy, so the baseline and the journal are measured the same
way.

## Interfaces / Schema

```text
toolu epic metrics [--epic <id>] [--since <iso>] [--json]
toolu epic summary <epic> [--json]
toolu epic baseline --state <dir> --until <iso> [--outlier <epic>]...
GET /api/v1/metrics[?epic=<id>&since=<iso>]
```

Without `--json`, `metrics` prints a short text table; with it, the document.
`summary` prints Markdown (`--json` gives `{schema, epic, rows, budgets,
tokens}`). `baseline` always prints JSON.

Metrics document, schema 1 (`docs/epic-orchestrator/metrics.schema.json`,
`additionalProperties: false` throughout). A `Pct` is
`{"count": n, "p50": ms|null, "p90": ms|null}`.

```json
{
  "schema": 1, "epic": "id|null", "since": "iso|null", "generated_at": "iso",
  "records": 0, "issues": 0,
  "lead_time_ms": "Pct: first launch-intent to cleanup-done",
  "phases": {"<phase>": {"ms": 0, "share": 0.0}},
  "ready_to_merged_ms": {
    "total": "Pct", "queue": "Pct", "rebase": "Pct", "ci": "Pct",
    "merge": "Pct", "rework": "Pct", "clean_excluding_ci": "Pct"
  },
  "rebase_rounds": {"engine": 0, "worker": 0, "issues": {"<key>": {"engine": 0, "worker": 0}}},
  "prompts": {"by_reason": {"<name>": 0}, "while_waiting": 0, "rebase_without_conflict": 0},
  "judgments": {"<kind>": {"raised": 0, "closed": 0, "wait_ms": "Pct"}},
  "wakeups": {"total": 0, "judgment_free": 0},
  "github": {
    "checks": 0, "rest_fresh": 0, "rest_not_modified": 0,
    "not_modified_share": "0..1|null", "rest_points": 0, "graphql_points": 0,
    "check_gap_ms": "Pct + max", "detection_bound_ms": "Pct"
  },
  "jobs": {"admissions": 0, "waits": 0, "pressure_holds": 0, "wait_ms": "Pct"},
  "launches": {"relaunches": 0, "host_switches": 0, "cooldowns": 0, "waits": 0},
  "engine": {"samples": 0, "max_rss_bytes": "n|null", "cpu_ms": "n|null"}
}
```

Definitions (each from journal records only):

- **Epic filter:** a record is in epic `E` when its `epic` is `E`, or its
  `key` is an issue key seen with epic `E`. `--since` keeps records whose `at`
  is at or after it.
- **Lead time:** first `launch-intent` to `cleanup-done` per issue.
- **Phases:** each `transition`/`phase` (note = phase) lasts until the next
  phase record of that issue, its `cleanup-done`, or `generated_at`.
- **Ready to merged:** from the first `phase` = `ready` (or
  `github-babysit-ready`) to the first merge marker (`github-pr-merged`, or a
  `merge-done` of a merged step). Time is split by state: `queue` until a
  `merge-intent`; a step's span goes to `rebase` if its `merge-done` outcome
  is `rebased`, to `merge` if it merged, else to `queue`; after `rebased`,
  `waiting` or `stale` the issue is in `ci` until the next `merge-intent`;
  after `handoff`, `fix` or `queue-left` it is in `rework` until it is ready
  again. `clean_excluding_ci` covers issues with no rework: total minus `ci`.
- **Prompts:** `prompt` records by name. `rebase_without_conflict`: a
  `prompt`/`rebase` not preceded, for that issue, by a `merge-done` with
  outcome `handoff` since the previous `rebase` prompt. `while_waiting`: a
  `babysit` or `status` prompt whose issue's latest `github-check` `next` is
  `keep-going` or `merge-queue`.
- **Judgments:** `judgment` records by name (`raised`); `closed` records give
  `kind=<k> waited_ms=<n>`.
- **Wake-ups:** `wait-reply` records; `judgment_free` are `timeout` replies.
- **GitHub:** `github-check` note fields summed; `check_gap_ms` between
  consecutive `github-check` records of one watch key whose `cause` is the
  schedule; `detection_bound_ms` is the gap before each check that saw a
  change (`restFresh` > 0), the measured window the change landed in.
- **Jobs:** `admission`/`job` records are attempts, `job-wait` refusals; a
  wait runs from the first refused attempt of a streak to the next attempt
  that was not refused; `pressure_holds` counts `job-wait` = `pressure`.
- **Launches:** `relaunches` = `launch-intent` beyond the first per issue,
  `host_switches` = `reroute`, `cooldowns` = `host-limited`, `waits` =
  `launch-wait`.
- **Engine:** the maximum `max_rss_bytes` and the last `cpu_ms` of
  `engine-usage` samples.

Budgets (`metrics::budgets`, each `{id, budget, value, status}` with status
`pass`, `fail` or `unmeasured`):

| id | Value | Budget |
|---|---|---|
| `judgment-free-wakeups` | `wakeups.judgment_free` | 0 |
| `prompts-while-waiting` | `prompts.while_waiting` | 0 |
| `rebase-without-conflict` | `prompts.rebase_without_conflict` | 0 |
| `verdict-reaction` | max `detection_bound_ms` plus the largest `github-babysit-ready` → next `merge-intent` gap, floored to seconds | ≤ 180 s |
| `check-gap` | max `check_gap_ms`, floored to seconds | ≤ 180 s |
| `ready-to-merged` | `clean_excluding_ci` p90 | ≤ 240 s |
| `idle-engine` | `engine.max_rss_bytes` | ≤ 10 MiB (timers: CI only) |

Baseline document (`benchmarks/epic-baseline.json`):

```json
{
  "schema": 1,
  "source": {"until": "2026-10-05T01:30:00Z", "outliers": ["falconiere-toolu-orm-145"], "records": 115, "merged": 98},
  "main": {"epics": 14, "merged_issues": 83, "agent_hours": 542.2,
           "lead_time_ms": "Pct", "ready_to_cleanup_ms": "Pct", "phases": {"<phase>": {"ms": 0, "share": 0.0}},
           "rebase_rounds": 119, "relaunched_issues": 11},
  "outliers": {"falconiere-toolu-orm-145": "same shape as main"}
}
```

An issue counts when `launched_at` ≤ `until`; it is merged when
`stage` = `merged` and `finished_at` ≤ `until`. Lead time is `launched_at` to
`finished_at`; `ready_to_cleanup` is the first `ready` in `status/<key>.json`
history to `finished_at`; phases run from each history entry to the next
entry or `finished_at`; rebase rounds count `rebasing` entries; relaunched
issues have `launches` > 1.

Token use (summary only), per issue: Claude Code — the issue's
`session_id` transcript under `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<slug>/`,
assistant messages deduplicated by `message.id` (last frame), summing
`input_tokens`, `output_tokens`, `cache_read_input_tokens`,
`cache_creation_input_tokens`; Codex — the rollout whose name ends in the
session id under `<CODEX_HOME or ~/.codex>/sessions/`, last
`token_count.info.total_token_usage`. Every row carries `source` and
`complete` (`false` when the issue launched more than once, since only the
last session id is recorded). Other hosts or a missing file give
`{"status": "unavailable", "reason": "..."}`.

## Failure modes and edge cases

- **Empty or absent journal directory:** an all-zero document with `null`
  percentiles; exit 0.
- **Unparseable record or note:** the line is skipped as `journal::page`
  already does; a note that is not JSON where JSON is expected counts the
  record but contributes no field.
- **Unknown epic:** `metrics --epic x` returns the zero document;
  `summary x` fails with exit 1 and `unknown epic x` when no record names it.
- **Bad `--since`/`--until`:** usage error, exit 2; HTTP 400 `bad_query`.
- **Journal older than the new records:** wake-ups, babysit `next`, job waits
  and engine samples are absent; their counts are 0 and the affected budgets
  are `unmeasured`, never `pass`.
- **Issue without a merge or cleanup:** excluded from lead time and ready to
  merged; its open phase counts up to `generated_at`.
- **Baseline state directory missing files:** a missing `status/<key>.json`
  drops that issue from ready and phase figures only; an unreadable JSON file
  fails the command with its path (exit 1), since a silently smaller baseline
  would be wrong.
- **Session logs missing or unreadable:** that row is `unavailable` with the
  reason; the summary still prints.
- **Concurrent writes:** readers take no lock; a partial last line is ignored
  (existing journal rule).
- **HTTP listener stop:** a connection accepted after stop is set is closed
  without being served.
- **Herdr stop while blocked:** the stream shutdown returns the read; a
  shutdown on a stream that already closed is ignored.

## Acceptance criteria

- **AC-1:** `toolu epic baseline --state ~/.claude/epics --until 2026-10-05T01:30:00Z --outlier falconiere-toolu-orm-145` prints JSON byte-identical to the committed `benchmarks/epic-baseline.json` (115 records, 98 merged, 83 main issues, 542.2 agent-hours, 119 rebase rounds, 11 relaunched).
- **AC-2:** On a committed fixture state directory, `toolu epic baseline` returns the hand-computed counts, percentiles, shares and outlier split; a malformed issue file fails with exit 1 naming it.
- **AC-3:** `toolu --json epic metrics` on a committed fixture journal validates against `docs/epic-orchestrator/metrics.schema.json` and equals hand-computed values for lead time, phases, the ready-to-merged split, rebase rounds, prompts, judgments, wake-ups, GitHub counts, job waits, launches and engine samples; `--epic` and `--since` narrow it.
- **AC-4:** `GET /api/v1/metrics` returns the same document as the CLI for the same journal; a bad `since` returns 400.
- **AC-5:** The scripted epic (scenario rig: launch, ready, queue with an engine rebase and a conflict hand-off, merge, close) produces a journal whose metrics pass every deterministic budget, in CI.
- **AC-6:** Each count budget fails on a planted regression: a babysit prompt on a keep-going check (`prompts-while-waiting`), a `REBASE` prompt without a hand-off (`rebase-without-conflict`), a `timeout` wait reply (`judgment-free-wakeups`); and the GitHub check schedule after a fresh, unchanged, failed or errored check is exactly 180 s later.
- **AC-7:** On Linux, an idle release engine with the HTTP listener stays at `VmHWM` ≤ 10 MiB and wakes each thread at most twice in a 3 s window; with the old 20 ms polling the same test fails.
- **AC-8:** The engine journals `wait-reply`, the babysit `next` on `github-check`, `job-wait` and `engine-usage` records in real runs (each covered by a test that drives the real path and reads the journal).
- **AC-9:** `toolu epic summary <epic>` prints lead time, phase shares, rebase rounds, ready-to-merged and relaunches beside the baseline, the budget table and token rows; Claude Code and Codex rows sum real transcript fixtures, OpenCode and Cursor rows say `unavailable`; the epic's closing comment carries the same summary.
- **AC-10:** `cargo xtask check-markdown-cli`, `cargo xtask docs-cli` drift, `cli-compat` and the full `cargo xtask gate` pass with the new verbs documented.

## Acceptance evidence

| AC | Input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | real `~/.claude/epics` | byte-identical file | outlier excluded from main | `toolu epic baseline … \| diff - benchmarks/epic-baseline.json` (local; recorded in PR) |
| AC-2 | `fixtures/epic/baseline/` | hand-computed JSON | missing status file, malformed issue | `cargo test -p toolu-epic-orchestrator metrics::baseline` |
| AC-3 | `fixtures/epic/metrics/journal/*.jsonl` | `fixtures/epic/metrics/expected.json`, schema-valid | empty journal, old journal without new names | `cargo test -p toolu-cli --test epic_metrics` |
| AC-4 | same fixture journal under a real engine | identical documents; 400 | bad query | `cargo test -p toolu-epic-orchestrator status_api` |
| AC-5 | scenario rig journal | every deterministic budget `pass` | conflict hand-off present | `cargo test -p toolu-epic-orchestrator metrics_scenario` |
| AC-6 | planted records / watch schedule | named budget `fail`; due = at + 180 000 | errors and 304s | `cargo test -p toolu-epic-orchestrator budgets` |
| AC-7 | release `toolu epic serve --http` | ≤ 10 MiB, ≤ 2 switches per thread | 3 s window | `cargo test -p toolu-epic-orchestrator --release --test idle_budget` (Linux CI `rust` job) |
| AC-8 | real wait, babysit check, refused job, engine tick | the records in the journal | timeout vs judgment | `cargo test -p toolu-epic-orchestrator` (dispatch, github_engine, job, server tests) |
| AC-9 | fixture Claude transcript and Codex rollout | summed tokens; `unavailable` rows | relaunched issue `complete: false` | `cargo test -p toolu-epic-orchestrator metrics::summary` |
| AC-10 | the branch | all gates green | — | `cargo xtask gate` |

## Documentation impact

- `docs/epic-status-api.md` (generated from `status_schema.rs`): the
  `/api/v1/metrics` row.
- `docs/epic-orchestrator/README.md`: a "Metrics and budgets" section
  (commands, definitions, budgets, the baseline and how to re-derive it).
- `docs/epic-orchestrator/metrics.schema.json`: new.
- `docs/cli/`: regenerated for `metrics`, `summary`, `baseline`.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`: the closing
  step mentions `toolu epic summary`.
- `benchmarks/README.md`: the `epic-baseline.json` entry.
- `AGENTS.md`: the `toolu-epic-orchestrator` key-file row names metrics.

## Open Questions

1. *Attaching the first real engine-run report to #451* — owner: the epic
   orchestrator after the first epic finishes on the engine; non-blocking.
   This PR provides `toolu epic summary`; the PR body says the fourth
   acceptance box is completed by that run.
2. *Exact 2026-10-04 percentiles* — resolved: the hand method was not
   recorded; the committed command (linear interpolation) is now the method,
   and its values sit next to the issue's in the PR body.
