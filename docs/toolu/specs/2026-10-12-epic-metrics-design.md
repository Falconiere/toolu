# Epic metrics, baseline and efficiency budgets — Design

**Date:** 2026-10-12   **Status:** Approved (revision 5; spec review rounds 1–4 by an independent reviewer, final finding resolved in Open Question 3)   **Author:** Claude (epic worker, #451)   **Topic:** Measure epic runs from the engine journal, against a derived TypeScript baseline, under CI budgets

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
2. No fleet view, no write API, no HTTP surface beyond `GET /api/v1/metrics`.
3. No estimated value presented as a measurement. Anything not recorded is
   `null` (or a budget `unmeasured`) with a reason.
4. No OpenCode or Cursor token reading (OpenCode keeps usage in SQLite, Cursor
   in no readable store); both are reported `unavailable`.
5. Prompt reasons are the engine's prompt names: `babysit` (fix and thread
   findings travel in one prompt, so they are not split), `rebase`
   (`REBASE`), `status` (`STATUS?`) and `answer`. Splitting fix from thread
   would need a new prompt shape; out of scope.
6. Attaching the first real engine-run report to #451 is an operational step
   after merge (Open Questions 1).

## Architecture

All code lives in `crates/epic-orchestrator`, except a usage probe in
`toolu-runtime` (the process owner).

- **`metrics/` module** (new), a fold over journal records in sequence order,
  holding only per-issue and per-watch accumulators, never the whole journal.
  Submodules: `stats` (linear-interpolated percentiles and shares), `scope`
  (epic filter), `timeline` (lead time, phases), `ready` (first `ready` to
  merged and its split), `counts` (prompts, judgments, wake-ups, GitHub, jobs,
  launches, engine), `document`, `budgets`, `baseline` (TypeScript state
  directories), `tokens` (session logs), `summary` (Markdown), `text` (the
  `metrics` table). The document function takes `now_ms` explicitly. The same
  `stats` code serves the journal and the baseline.
- **`journal::each`** (new): stream retained records whose file day is at or
  after a given day, in file then line order, to a callback; `page` keeps its
  behaviour.
- **Journal additions.** The record shape is unchanged; only new names and one
  note field:
  1. `action`/`wait-reply` when a blocking `toolu epic wait`
     (`max_seconds` > 0) returns: note `judgment:<kind>` (key = the item's
     issue or epic key) or `timeout` (empty key). A `max_seconds` = 0 poll is
     not a wake-up and is not journaled. Written through the engine outbox
     after the reply is sent, and flushed in the same state-loop iteration
     (the reply paths `reply_wait`, `satisfy` and `expire` flush); a journal
     failure never withholds the reply.
  2. `github-check` and `github-error` notes gain `"issue"` (the issue key of
     a `Pr` watch, else absent) and `"nextAt"` (the watch's next scheduled
     slot after this attempt, Unix ms); `github-check` also gains `"next"`
     (the babysit decision: `keep-going`, `merge-queue`, `prompt`,
     `escalate`, `attention`; absent when no babysit ran). `server::stamp` resolves a
     `Pr` watch key to its issue's epic, so these records carry the epic.
  3. `admission`/`job-wait` when `toolu epic job` is refused: note `capacity`
     or `pressure` (a refusal starting `resource hold:`), key = the binding
     key (the issue key), epic empty; the fold scopes it by key like
     `admission`/`job`.
  4. `action`/`engine-usage`, note
     `{"event":"start"|"sample"|"stop","max_rss_bytes":n,"cpu_ms":n}` from
     `getrusage(RUSAGE_SELF)` through `toolu_runtime::process::self_usage()`,
     which normalizes `ru_maxrss` (KiB on Linux, bytes on macOS). Written at
     engine start, from the 30 s tick when 15 minutes passed since the last
     sample, and at a clean stop.
- **Idle timers.** Measured today: the HTTP listener polls a non-blocking
  `accept` every 20 ms (`http.rs` `accept_loop`); the herdr thread reads with a
  1 s timeout (`socket_herdr.rs`, `source.rs` `read_events`) and, with no
  herdr socket, sleeps 1 s at a time in `pause(stop, 30)`. Changes:
  - The HTTP listener blocks in `accept`; `join_threads` already connects to
    wake it; a connection accepted once stop is set is closed unserved.
  - A shared `Interrupt` (a `Mutex<Option<UnixStream>>` clone of the live herdr
    stream plus a `Condvar`) lets stop and resubscribe shut the stream down and
    wake `pause`. The herdr idle read timeout becomes 30 s, and `pause` waits
    on the condvar for its whole duration. `join_threads` triggers it before
    joining. `source::session` checks stop before each reconnect, and again
    right after publishing the new stream to the `Interrupt` (stop may land
    between connect and publish), so a stop never waits for a 30 s read. A
    failed session still retries after 1 s (a fault path, not an idle
    timer).
  - An idle engine (no epic, no client) then wakes on its 30 s tick and on the
    herdr 30 s pause or idle read only.
- **Surfaces.** `toolu epic metrics` (text: each measure beside the baseline;
  JSON: the document) and `GET /api/v1/metrics` call the same
  `metrics::document`; the CLI reads the journal directly (no engine needed).
  The HTTP route parses `epic` and `since` from the query and caches the
  document per query for 60 s, at most 16 entries (the oldest is evicted). `toolu epic summary` renders the document
  beside the baseline (embedded with
  `include_str!("../../../../benchmarks/epic-baseline.json")` from `src/metrics/`), the budget table
  and token rows. `close::summary_table` appends the same Markdown, without
  token rows, to the epic's closing comment. `toolu epic baseline` derives
  `benchmarks/epic-baseline.json`.
- **Budgets in CI.** A scripted-epic test computes the document from the
  scenario rig's real journal and asserts the deterministic budgets
  (enumerated below); planted-regression tests make each count budget fail;
  `watch_test` proves the 180 s schedule. A Linux integration test
  (`tests/idle_budget.rs`, same harness as `tests/status_budget.rs`) asserts
  the idle engine.

Decisive trade-off: one metrics implementation in the engine crate (Jev 0.59)
over an `xtask` copy, so the baseline and the journal are measured the same
way.

## Interfaces / Schema

```text
toolu epic metrics [--epic <id>] [--since <date>]       (global --json for the document)
toolu epic summary <epic>                               (global --json for {schema, epic, rows, budgets, tokens})
toolu epic baseline --state <dir> --until <iso> [--outlier <epic>]...   (always JSON)
GET /api/v1/metrics[?epic=<id>&since=<date>]
```

`<date>` is `YYYY-MM-DD` (UTC midnight) or an ISO-8601 UTC timestamp.

Metrics document, schema 1 (`docs/epic-orchestrator/metrics.schema.json`,
`additionalProperties: false`). `Pct` is
`{"count": n, "p50": ms|null, "p90": ms|null, "max": ms|null}`.

```json
{
  "schema": 1, "epic": "id|null", "since": "iso|null", "generated_at": "iso",
  "records": 0, "issues": 0,
  "lead_time_ms": "Pct",
  "phases": {"<phase>": {"ms": 0, "share": 0.0}},
  "ready_to_merged_ms": {"total": "Pct", "queue": "Pct", "rebase": "Pct", "ci": "Pct",
                         "merge": "Pct", "rework": "Pct", "clean_excluding_ci": "Pct"},
  "rebase_rounds": {"engine": 0, "worker": 0, "issues": {"<key>": {"engine": 0, "worker": 0}}},
  "prompts": {"by_reason": {"<name>": 0}, "while_waiting": 0, "rebase_without_conflict": 0},
  "judgments": {"<kind>": {"raised": 0, "closed": 0, "wait_ms": "Pct"}},
  "wakeups": {"total": 0, "judgment_free": 0},
  "github": {"checks": 0, "errors": 0, "rest_fresh": 0, "rest_not_modified": 0,
             "not_modified_share": "0..1|null", "rest_points": 0, "graphql_points": 0,
             "next_gap_ms": "Pct", "check_gap_ms": "Pct", "reaction_ms": "Pct",
             "late_reactions": 0},
  "jobs": {"attempts": 0, "waits": 0, "pressure_holds": 0, "wait_ms": "Pct"},
  "launches": {"relaunches": 0, "host_switches": 0, "cooldowns": 0, "waits": 0},
  "engine": {"lifetimes": 0, "max_rss_bytes": "n|null", "cpu_ms": "n|null"}
}
```

Definitions, journal records only:

- **Scope.** A record is in epic `E` when its `epic` is `E`, or its key is an
  issue key already seen with epic `E`. Machine-wide records with an empty
  key (`wait-reply` `timeout`, `engine-usage`) count in every scope: the
  orchestrator and the engine serve every epic at once. `--since` drops records before it.
  An interval that starts before `--since` (or before retention) is not
  counted, so lead time and ready-to-merged count only issues whose start
  record is in scope.
- **Lead time:** first `launch-intent` of the issue to its first
  `cleanup-done` with an empty note (a non-empty note is a failed or held
  cleanup).
- **Phases:** the issue's phase changes on `transition`/`phase` (note = the
  phase), on `github-babysit-ready` (phase `ready`) and on `queue-left` (note
  `<phase>: <text>`, phase = the text before the first `: `). Each phase lasts
  until the next change, the issue's `cleanup-done`, or `now`.
- **Ready to merged:** starts at the first entry into `ready`; ends at the
  first `github-pr-merged` or the first `merge-done` with an empty note (a
  merged step journals no outcome). A `merge-done` note's outcome is the text
  before the first `: `. Buckets: the issue starts in `queue`; a step
  (`merge-intent` to `merge-done`) is credited when it ends: `rebased` →
  `rebase`; empty (merged) → `merge`; anything else (`waiting`, `stale`,
  `busy`, `disarmed`, `error`, `closed`, `uncertain`, `refused`, `fix`,
  `handoff`) → `queue`. After `rebased`, `waiting` or `stale` the issue is in
  `ci` until its next `merge-intent`; after `handoff`, `fix` or a
  `queue-left` it is in `rework` until it enters `ready` again; other
  outcomes keep it in `queue`. `clean_excluding_ci` covers issues that never
  entered `rework`: their total minus `ci`.
- **Rebase rounds:** `rebase` records by `by` (`engine` | `worker`). The
  baseline's 119 rounds were all worker rounds (`rebasing` history entries);
  `worker` is the comparable figure.
- **Prompts:** `prompt` records by name. `rebase_without_conflict`: a
  `rebase` prompt for an issue with no `merge-done` outcome `handoff` since
  that issue's previous `rebase` prompt. `while_waiting`: a `babysit` prompt
  not licensed by a `github-check` of that issue with `next` `prompt` or
  `escalate` since the issue's previous `babysit` prompt (a held prompt keeps
  its license until sent), plus a `status` prompt whose issue's latest
  `github-check` `next` is `keep-going` or `merge-queue`.
- **Judgments:** for each `judgment` name outside the control names
  (`reopened`, `closed`, `acknowledged`, `answered`, `reminder`), `raised`
  counts distinct tokens. A `closed` record's note `kind=<k> waited_ms=<n>`
  adds to `closed` and `wait_ms` of kind `<k>`.
- **Wake-ups:** `wait-reply` records; `judgment_free` counts `timeout`.
- **GitHub:** `checks` = `github-check`, `errors` = `github-error`; REST and
  GraphQL fields summed from both notes; `not_modified_share` =
  not-modified / (fresh + not-modified), `null` when both are 0.
  `next_gap_ms`: per attempt, `nextAt` minus the attempt's `at` — the
  interval the engine scheduled, which a backoff would lengthen. Slots are
  whole 180 s steps by construction (`Watch::take_due`), so this budget
  catches a changed interval or a new backoff, not an observed delay; the
  README says so.
  `check_gap_ms` (reported, no budget): the actual gap between consecutive
  `scheduled` attempts of one watch key in one engine lifetime (no
  `engine-usage` `start` between) whose earlier attempt has a null
  `retryAfter`/`graphqlRetryAfter`; it also grows when a watch is dropped and
  re-added (a hand-off to `rebasing`) or another watch's rate limit holds all
  of GitHub, so it is context, not a budget. `reaction_ms`: from each
  `github-babysit-ready` (the check that moved the issue into `ready`) to the
  issue's next `merge-intent`. `late_reactions` counts those whose
  `merge-intent` came only after the issue's next `github-check`, unless
  another issue of the same repository had a `merge-intent` in between (the
  merge queue is per repository, so it was busy: queue wait, not reaction).
  An issue's repository comes from the `pr:<repo>#<n>` watch key of the
  `github-check` records that name it in `issue`.
- **Jobs:** per key in sequence order, `admission`/`job` opens an attempt and
  a `job-wait` of that key marks the latest open attempt refused. A wait runs
  from the first refused attempt of a streak to the next attempt not refused.
  `pressure_holds` counts `job-wait` notes `pressure`.
- **Launches:** `relaunches` = `launch-intent` records beyond the first per
  issue; `host_switches` = `reroute`; `cooldowns` = `host-limited`; `waits` =
  `launch-wait` records whose note is not a bare number (the streak-end
  record carries the count).
- **Engine:** `lifetimes` = `engine-usage` `start` records; `max_rss_bytes` =
  the largest sample; `cpu_ms` = the sum, over lifetimes, of each lifetime's
  last sample.

Budgets (`metrics::budgets`, rows `{id, budget, value, status, reason}`,
status `pass` | `fail` | `unmeasured`). A row is `unmeasured` when its records
are absent (for example a journal older than this change).

| id | Value | Budget | In the scripted-epic CI test |
|---|---|---|---|
| `judgment-free-wakeups` | `wakeups.judgment_free` | 0 | asserted (the test waits on the engine through a real `wait`) |
| `prompts-while-waiting` | `prompts.while_waiting` | 0 | asserted (a PR waits through keep-going checks) |
| `rebase-without-conflict` | `prompts.rebase_without_conflict` | 0 | asserted (engine rebase plus one conflict hand-off) |
| `verdict-reaction` | `github.late_reactions` | 0 (so verdict to reaction is at most one check interval, given `check-gap`) | asserted |
| `check-gap` | `next_gap_ms.max` | ≤ 180 000 ms | asserted (every rig check schedules its next slot); `watch_test` proves the slot arithmetic |
| `ready-to-merged` | `clean_excluding_ci.p90` | ≤ 240 s | asserted |
| `idle-engine` | — | ≤ 10 MB (10 000 000 bytes) resident, no timer under 30 s | always `unmeasured` in the document, reason `asserted by tests/idle_budget.rs`; `engine.max_rss_bytes` is a lifetime peak, not an idle figure |

Baseline document (`benchmarks/epic-baseline.json`):

```json
{
  "schema": 1,
  "source": {"until": "2026-10-05T01:30:00Z", "outliers": ["falconiere-toolu-orm-145"], "records": 115, "merged": 98},
  "main": {"epics": 14, "merged_issues": 83, "agent_hours": 542.2,
           "lead_time_ms": "Pct", "ready_to_cleanup_ms": "Pct",
           "phases": {"<phase>": {"ms": 0, "share": 0.0}},
           "rebase_rounds": 119, "relaunched_issues": 11},
  "outliers": {"falconiere-toolu-orm-145": "same shape as main"},
  "reported": {"date": "2026-10-04", "lead_time_min": {"p50": 219, "p90": 912},
               "first_ready_to_cleanup_min": {"p50": 1, "p90": 69},
               "supervisor_rss_mb_per_epic": 31,
               "note": "hand-computed for #451 and the engine brainstorm; method not recorded"}
}
```

`--until` and `--outlier` are the command's inputs; `reported` is copied
verbatim from #451 by the command (a constant), so the derived and hand
figures sit side by side. An issue counts when `launched_at` ≤ `until`; it is
merged when `stage` = `merged` and `finished_at` ≤ `until`. Lead time is
`launched_at` to `finished_at`; agent-hours its sum; `ready_to_cleanup` is the
first `ready` in `status/<key>.json` history to `finished_at`; phases run from
each history entry at or before `finished_at` to the next or `finished_at`;
rebase rounds count `rebasing` entries; relaunched issues have `launches` > 1.
The cutoff `2026-10-05T01:30:00Z` is the state on the evening of 2026-10-04
(UTC−3): after the merge at 01:23:28Z, which the issue's 98 include, and
before the next at 02:00:37Z.

Token use (`summary` only), per issue with a `session_id` in its record:
Claude Code — `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<slug>/<session>.jsonl`
plus `<slug>/<session>/subagents/*.jsonl`, `<slug>` being the worktree path
with every non-alphanumeric character replaced by `-` (as `launch/session.rs`
finds it); assistant messages deduplicated by `message.id`, last frame kept,
summing `input_tokens`, `output_tokens`, `cache_read_input_tokens`,
`cache_creation_input_tokens`. Codex — the rollout file under
`<CODEX_HOME or ~/.codex>/sessions/` whose name ends in `<session>.jsonl`, its
last `token_count` `info.total_token_usage`. Each row has `source` and
`complete` (`false` when the issue launched more than once: only the last
session id is recorded). OpenCode, Cursor, no session id or no file give
`{"status": "unavailable", "reason": <fixed phrase>}`; reasons never contain a
path.

## Failure modes and edge cases

- **Empty or absent journal directory:** zero counts, `null` percentiles,
  every budget `unmeasured`; exit 0, HTTP 200.
- **Unparseable line or note:** a bad line is skipped (existing rule); a note
  that should be JSON but is not counts the record and contributes no field.
- **Unknown epic:** `metrics --epic x` returns the zero document; `summary x`
  fails with exit 1 and `unknown epic x` when no record names it.
- **Bad `--since`/`--until`:** usage error, exit 2; HTTP 400 `bad_query`.
- **Journal older than the new records:** the affected counts are 0 and their
  budgets `unmeasured`, never `pass`.
- **Issue without a merge or cleanup:** excluded from lead time and ready to
  merged; its open phase counts up to `now`.
- **Baseline:** a missing `status/<key>.json` drops that issue from ready and
  phase figures only; an unreadable or malformed issue or status file fails
  with its path (exit 1) — a silently smaller baseline would be wrong; a
  missing `--state` directory fails (exit 1).
- **Session logs missing or unreadable:** that row is `unavailable`; the
  summary still prints.
- **Concurrent writes:** readers take no lock; a partial last line is ignored.
- **Large journals:** the fold keeps per-issue and per-watch state only; the
  HTTP route recomputes at most once per 60 s per query.
- **HTTP stop:** a connection accepted after stop is closed unserved.
- **Herdr stop or resubscribe while blocked:** the stream shutdown ends the
  read at once; shutting down an already closed stream is ignored.

## Acceptance criteria

- **AC-1:** Given the recorded `~/.claude/epics`, `toolu epic baseline --state ~/.claude/epics --until 2026-10-05T01:30:00Z --outlier falconiere-toolu-orm-145` prints JSON identical to the committed `benchmarks/epic-baseline.json`, whose `main` has 83 merged issues, 542.2 agent-hours, lead p50 219.9 min, 119 rebase rounds and 11 relaunched issues.
- **AC-2:** Given the committed fixture state directory `fixtures/epic/baseline/`, `toolu epic baseline` returns the hand-computed counts, percentiles, shares and outlier split; a malformed issue file fails with exit 1 naming it.
- **AC-3:** Given the committed fixture journal `fixtures/epic/metrics/journal/`, in which every issue has its `cleanup-done` so no value depends on the clock, `toolu --json epic metrics` validates against `docs/epic-orchestrator/metrics.schema.json` and equals `fixtures/epic/metrics/expected.json` (all fields but `generated_at`); `--epic` and `--since` narrow it to their own hand-computed expectations; the text form shows lead time, phase shares and rebase rounds beside the baseline.
- **AC-4:** Given the same fixture journal under a running engine, `GET /api/v1/metrics` returns the CLI's document (all fields but `generated_at`); `?since=bad` returns 400.
- **AC-5:** Given the scripted epic (scenario rig: launch, ready, a PR waiting through keep-going checks, an engine rebase, one conflict hand-off, merges, close, and a real blocking `wait`), the metrics of its journal mark `judgment-free-wakeups`, `prompts-while-waiting`, `rebase-without-conflict`, `verdict-reaction`, `check-gap` and `ready-to-merged` as `pass`.
- **AC-6:** Given planted journals, each count budget fails: a `babysit` prompt after only keep-going checks (`prompts-while-waiting`), a `rebase` prompt with no hand-off (`rebase-without-conflict`), a `timeout` wait reply (`judgment-free-wakeups`), a `merge-intent` after a second check with no other merge between (`verdict-reaction`), a check whose `nextAt` is 360 s out (`check-gap`); and given a watch, the next scheduled slot after a fresh, unchanged, failed or errored check is exactly 180 000 ms after the previous slot.
- **AC-7:** On Linux, a release `toolu epic serve --http` with no epic, after 2 s of warm-up, keeps `VmHWM` ≤ 10 000 000 bytes (the issue's 10 MB; the measured idle figure is recorded in Open Questions 3) and makes at most 20 voluntary context switches in total, summed over `/proc/<pid>/task/*/status`, in a 35 s window, with the herdr thread running and no herdr socket. Each idle source (the 30 s tick with its journal and watch I/O, the herdr 30 s pause and its outage message) fits at most twice in the window; so the idle total is about 4 wake-ups plus their I/O waits, while the 20 ms accept poll gives about 1 750 and a single 1 s timer (today's herdr `pause`) about 35, both failing it. The limit was not measured on Linux before the change; the CI test's first run records the figure in the PR, and a limit change needs that evidence. Unit tests pin the idle constants: the herdr idle read and pause are 30 s, and `wake_after` on an idle world waits for the tick.
- **AC-8:** Given a real blocking `wait` that times out and one that returns a judgment, a babysit check, a refused `toolu epic job`, and an engine start, tick and stop, the journal holds `wait-reply` (`timeout`, `judgment:<kind>`), a `github-check` with `issue` and `next` and the issue's epic, `job-wait` (`capacity`), and `engine-usage` `start`/`stop` with `max_rss_bytes` > 0.
- **AC-9:** Given fixture Claude Code (with a subagent file) and Codex transcripts, `toolu epic summary <epic>` prints lead time, phase shares, rebase rounds, ready-to-merged and relaunches beside the baseline, the budget table, summed token rows, `unavailable` rows for OpenCode and Cursor, and `complete: false` for a relaunched issue; the epic's closing comment carries the summary without token rows.
- **AC-10:** `cargo xtask check-markdown-cli`, the `docs-cli` drift check, `cli-compat` and the full `cargo xtask gate` pass with the new verbs documented.

## Acceptance evidence

| AC | Input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | real `~/.claude/epics` | identical output | outlier only in `outliers` | `toolu epic baseline … \| diff - benchmarks/epic-baseline.json`, run locally; output recorded in the PR |
| AC-2 | `fixtures/epic/baseline/` | hand-computed JSON | missing status file; malformed issue file | `cargo test -p toolu-cli --test epic_baseline` |
| AC-3 | `fixtures/epic/metrics/journal/` | `expected.json`, schema-valid | empty journal; journal without the new names; `--since` cutting an interval; an open phase (unit test with a fixed `now_ms`) | `cargo test -p toolu-cli --test epic_metrics` and `cargo test -p toolu-epic-orchestrator metrics::` |
| AC-4 | same journal under `serve_with_http` | same document; 400 | bad query | `cargo test -p toolu-epic-orchestrator status_api` |
| AC-5 | scenario rig journal | five budgets `pass` | conflict hand-off present | `cargo test -p toolu-epic-orchestrator metrics_scenario` |
| AC-6 | planted records; `Watch` | named budget `fail`; slot + 180 000 | errors, 304s, late wake | `cargo test -p toolu-epic-orchestrator budgets` and `watch` |
| AC-7 | release binary | ≤ 10 MB, ≤ 20 switches in 35 s | herdr socket absent | `cargo test -p toolu-epic-orchestrator --release --test idle_budget` (Linux `rust` CI job) |
| AC-8 | real engine paths | the records | timeout and judgment | `cargo test -p toolu-epic-orchestrator` (dispatch, github_engine, job, server tests) |
| AC-9 | transcript fixtures | token sums; `unavailable` | relaunched issue | `cargo test -p toolu-epic-orchestrator metrics::` and `close` |
| AC-10 | the branch | green | — | `cargo xtask gate` |

## Documentation impact

- `docs/epic-status-api.md` (generated from `status_schema.rs`): the
  `/api/v1/metrics` row.
- `docs/epic-orchestrator/README.md`: "Metrics and budgets" (commands,
  definitions, budgets, the baseline and how to re-derive it).
- `docs/epic-orchestrator/metrics.schema.json`: new.
- `docs/cli/`: regenerated for `metrics`, `summary`, `baseline`.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`: the closing
  step names `toolu epic summary`.
- `benchmarks/README.md`: the `epic-baseline.json` entry.
- `AGENTS.md`: the `toolu-epic-orchestrator` key-file row names metrics.

## Open Questions

1. *First real engine-run report on #451* — owner: the epic orchestrator,
   after the first epic finishes on the engine; non-blocking. The PR body
   states that the fourth acceptance box is completed by that run with
   `toolu epic summary`.
2. *2026-10-04 percentiles* — resolved: the hand method was not recorded; the
   committed command (linear interpolation) is the method of record, and the
   hand figures are kept under `reported`.
3. *Idle engine figures* — resolved by measurement on 2026-10-12 (macOS,
   release `toolu epic serve --http` from `main` at 9d593b01, no epic, no
   herdr socket): resident 5.2 MB after 3 s and 13 s; `top` counted 874
   context switches in a 5 s sample, from the 20 ms accept poll and the 1 s
   herdr pause. The 10 MB budget therefore holds with margin; the Linux CI
   test confirms `VmHWM` on its runner, and only the timers need the fix.
   The Linux switch figure is not measured yet: the first CI run of
   `tests/idle_budget.rs` prints the measured `VmHWM` and switch total, which
   go into the PR body, and changing the 20-switch or 10 MB limit later needs
   that evidence. Owner: this PR; non-blocking for planning.
