# Epic metrics, baseline and efficiency budgets (#451)

Brainstorm, 2026-10-12, Delivery mode, Full path (public CLI and HTTP
interface, new journal records, engine listener change).

## Capsule

- **Outcome:** epic runs on the resident engine are measured from its journal
  (`toolu epic metrics`, `/api/v1/metrics`, `toolu epic summary`), compared
  with a baseline derived by a committed command from the TypeScript-era state
  directories (`benchmarks/epic-baseline.json`), and held by budgets: asserted
  in CI on the scripted epic, reported for real runs.
- **Material defaults:**
  - One metrics implementation in `toolu-epic-orchestrator`. The baseline is
    derived by `toolu epic baseline --state <dir> --until <ts> --outlier <epic>`,
    which reuses the same percentile and phase-share code as the journal
    metrics (Jev 0.59 over a `cargo xtask` task; a Bun script is ruled out by
    #440).
  - Percentiles are linear-interpolated. Durations are milliseconds.
  - The journal gains only the records the metrics cannot derive today:
    `wait` replies (orchestrator wake-ups), each babysit decision, job
    admission refusals, and engine resource samples.
  - The HTTP and control listeners stop polling `accept` every 20 ms; an idle
    engine then has no timer under 30 s, which a Linux test asserts together
    with 10 MB resident (Jev 0.71 for fix-and-measure over static checks).
  - Token use: Claude Code session logs and Codex rollouts are read for the
    issue's captured session; OpenCode keeps usage in SQLite and Cursor keeps
    none we can read, so both are reported `unavailable` with the reason
    (Jev 0.87). Tokens appear in `summary` only, not in every API poll.
- **Non-goal:** a fleet view, changing what the engine does, estimating any
  value that is not recorded, or attaching the first real epic report (that
  happens after the first epic runs on the engine).
- **Repository evidence:**
  - The journal (`src/journal.rs`) already records phase transitions, action
    intents and outcomes, queue steps (`merge-done`, `rebase` with
    `by: engine|worker`), prompts by name, judgments with `waited_ms` on close,
    `github-check` notes with `restNotModified`/`restFresh`/points, `reroute`,
    `host-limited`, `launch-wait`, and job `admission`.
  - The scripted epic exists: `src/tests/scenario_rig_test.rs` and
    `queue_rig_test.rs` drive launch, ready, queue, merge and close against
    the HTTPS GitHub fixture and the herdr fixture.
  - `tests/status_budget.rs` already reads `VmHWM` on Linux.
  - Baseline probe over `~/.claude/epics` with cutoff
    `2026-10-05T01:30:00Z` and `falconiere-toolu-orm-145` as outlier gives
    exactly the issue's 115 records, 98 merged, 83 non-outlier, 542 agent-hours,
    lead p50 219.9 min, 119 rebase rounds and 11 relaunched issues. Lead p90
    (925 vs 912 min) and first-ready-to-cleanup p90 (87 vs 69 min) differ
    because the 2026-10-04 percentile method was not recorded; the committed
    command is now the method of record.
- **Risk:** journal growth (one extra line per babysit check and per 15-minute
  usage sample); listener shutdown semantics after removing the polling;
  wall-clock budgets on the scripted epic run in real time, so time budgets
  there are checked through the schedule rather than elapsed seconds.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Interface | `toolu epic metrics`, `summary`, `baseline`; `/api/v1/metrics` | `status`/`queue` verbs and `status_api::route` | Each verb and flag becomes a compatibility contract (cli-compat) |
| Persistence | New journal names in the existing closed record shape | `journal::Record` | Old journals lack the new names; metrics report those measures as absent, not zero |
| Cost | Metrics read retained journal files on request | `journal::page` reads every file | 90 days of journal per request; bounded by `--since` and the epic filter |
| Honesty | Absent data is `null` with a reason | issue: "no estimates presented as measurements" | Change-detection latency is reported as the measured check interval that contained the change, labelled as a bound |

## Alternatives rejected

- `cargo xtask epic-baseline`: a second percentile and phase-share
  implementation that can drift from the journal metrics.
- A Bun baseline script: TypeScript tooling is being removed (#440).
- Asserting only the configured constants for the idle budget: the 20 ms
  accept polling would stay while the budget claimed otherwise.
- Reading OpenCode's SQLite database: a new native dependency for one host;
  revisit with `opencode export` if needed.
- Token use in `/api/v1/metrics`: reading every session log on each poll.
