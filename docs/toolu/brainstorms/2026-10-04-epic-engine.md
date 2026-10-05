# Epic engine: an efficient, autonomous epic-orchestrator with a status server

Brainstorm, 2026-10-04. Part of the Rust rebuild (epic #402). The user called epic-orchestrator the most valuable plugin, asked that it be efficient and effective, and asked for an HTTP server to consult the status of running tasks and hosts. A follow-up asked whether babysit can react the moment a code review finishes, and then ruled out introducing a tunnel or a GitHub App and fixed the babysit check at every 3 minutes.

## Capsule

- **Outcome:** `toolu epic` runs as a resident engine that drives an epic to done on its own. It gates, queues, rebases, merges, cleans up and launches the next issue without an LLM turn, and wakes the orchestrator LLM only for judgment. It reacts to pushed local events (herdr socket, worker reports), checks GitHub every 3 minutes with requests that cost nothing while nothing changes, keeps an append-only journal, and serves a read-only HTTP status API with a live dashboard.
- **Material defaults:**
  - Autonomous engine (user decision), inside today's authorization boundary.
  - Per-repository merge queue with mechanical rebase of the head PR only.
  - Babysit ticks run in the engine every 3 minutes until everything is green; the worker LLM is prompted only when there is something to fix.
  - The 3-minute interval is fixed and never increases (user decision): no backoff, no adaptive schedule.
  - No tunnel, GitHub App or webhook (user decision).
  - Journal (JSON lines) plus today's JSON records as snapshots.
  - HTTP server: read-only, loopback by default, bearer token for any other bind.
- **Non-goal:** changing what the engine is allowed to do, relaxing the "head contains base tip" rule, a write API over HTTP, stacked merge trains, webhooks of any kind.
- **Repository evidence:** 115 issue records in the real state directories under `~/.claude/epics/`, the watcher and skill source, herdr's socket API schema, GitHub's REST and webhook documentation, and a conditional-request measurement on PR #454.
- **Risk:** an engine that merges unattended must fail closed and be easy to pause; polling must stay inside GitHub's rate limits; mechanical rebase must never run while a worker is active in the worktree.
- **Handoff:** epic #402, phase 5 sub-issues.

## Evidence: where the time goes today

Source: every `issues/*.json` and `status/*.json` under `~/.claude/epics/` on 2026-10-04. 115 issue records, 98 merged, 15 epics with merged issues. One epic (toolu-orm-145) is reported separately because its 15 issues each sat 30 to 45 hours between `ready` and cleanup and would dominate every total. The rest: 83 merged issues, 542 agent-hours.

| Measure | Value |
|---|---|
| Lead time, launch to cleanup | p50 219 min, p90 912 min |
| Time from first `ready` to cleanup | p50 1 min, p75 3 min, p90 69 min |
| Rebase rounds | 119, across 38 of 83 issues (worst epic: 61 rounds for 22 issues) |
| Relaunched issues | 11 of 83 (up to 6 launches for one issue) |

| Phase | Share of wall time |
|---|---|
| execution | 38.5% |
| babysit | 22.8% |
| needs-human | 16.0% |
| ready (worker done, waiting for the orchestrator) | 11.1% |
| rebasing | 3.6% |
| brainstorm, spec, plan, reviews, pr-open | 7.1% |

What the code does, read from `scripts/epic-watch.ts`, the skill and the babysit workflow:

- **The LLM is the control loop.** The watcher polls `herdr agent list` and the status files every 60 seconds, then exits on any event, which re-invokes the orchestrator LLM. The LLM then runs deterministic scripts by hand: merge gate, merge, cleanup, graph, route, launch, restart the watcher. If its session ends, compacts or hits a usage limit, nothing advances. That is the `ready` share, and the outlier epic.
- **Wake-ups with no decision in them.** `recheck` wakes the LLM every 5 minutes while a PR awaits merge, and `heartbeat` every 45 minutes, only to re-run a script.
- **Babysit is an LLM loop.** On Claude Code a cron prompt fires every 3 minutes (adaptive backoff) in the worker's session; on Codex and OpenCode the worker waits at most 60 seconds between ticks. Most ticks only learn that CI or the review is still running. Babysit took about 124 agent-hours here. At the base cadence that is up to roughly 2,500 LLM turns; the real number is lower with backoff and is not recorded anywhere.
- **Rebases go through the worker LLM.** Every merge moves main, so each other open PR gets `REBASE` at its own gate: the worker rebases, re-runs the local gate, force-pushes and babysits again.
- **No journal.** State is the latest record per issue plus a phase history. Events, wake-ups, gate verdicts over time, job-admission waits and token use are not recorded, so none of the above can be tracked or budgeted.
- **The watcher is a resident Bun process**, about 31 MB each (two were running).

Slot utilisation is close to the cap in the larger epics, so idle slots are not the main loss.

## Design

### Engine

One resident process per machine (per resource root), `toolu epic engine`, started on demand by `toolu epic start <epic>` and independent of any LLM session. Jev: 0.99 for this over porting the LLM loop.

- **State machine per issue**, driven by events. Deterministic transitions run in the engine:

  | Event today | Who acts today | Engine |
  |---|---|---|
  | `ready` | LLM runs the gate, then merge, then cleanup | gate → merge queue → merge → cleanup → graph → route → launch |
  | `recheck`, `heartbeat` | LLM re-runs scripts | internal timers and events, no wake-up |
  | `gone` | LLM relaunches | snapshot, relaunch up to 2 times, then escalate |
  | `host-limited` | LLM reroutes and replaces | cooldown, reroute, replace |
  | `gh-budget-low` | LLM holds launches | engine holds itself until reset |
  | `stalled` | LLM sends `STATUS?` | sends `STATUS?` once, escalates if no progress |
  | `needs-human`, `failed`, `blocked` | LLM or user | judgment event |

- **Judgment events** are the only thing the LLM sees: `needs-human`, `failed`, `blocked`, a stall that survived a nudge, a dependency cycle, a coupling hold, incomplete cleanup, a relaunch limit. The orchestrator skill runs `toolu epic wait`, which blocks until one arrives. With no LLM waiting, the events sit in the attention queue and the engine keeps doing everything else.
- **Authorization is unchanged.** The engine acts only on registered epics, never pushes to the base branch, merges only gate-green PRs pinned to the verified head SHA, and uses `--admin` only for a protection-only refusal. `toolu epic pause` (one epic or all) stops launches and merges at once and is persisted.
- **Launch safety without the LLM.** The hidden-coupling check becomes a Jev score over the batch's titles and bodies. A confident overlap holds the lower-priority issue and raises `coupling-hold`; everything else launches.
- **No async runtime.** Threads and channels: one thread per event source feeding one state-machine thread. Target: 10 MB or less resident when idle, and no timer shorter than 30 seconds while nothing is happening.

### Event sources

| Source | Today | Engine |
|---|---|---|
| Agent state | `herdr agent list` every 60 s | herdr socket `events.subscribe` (`pane.agent_status_changed`, `pane.exited`, `pane.closed`, `worktree.removed`) |
| Worker phase | status file read every 60 s | `toolu epic report` sends to the engine's socket; the file is kept as a spool when the engine is down |
| CI, review, threads, merges, base moved | LLM cron tick every 3 min, backing off | engine check every 3 min, fixed, no LLM turn |
| Issues closed or reopened outside the run, new sub-issues, dependencies | graph re-run every 45 min | the same 3-minute check |

Local sources are pushed. GitHub is polled, because pushing from GitHub needs a way in to the machine.

### Babysit: a check every 3 minutes, without an LLM turn

The review bot is a GitHub Action that posts its verdict as a PR comment. GitHub can push that as a webhook (`issue_comment`), but a webhook has to reach the machine, which means a public address, a tunnel or a relay, and the user ruled out a tunnel and a GitHub App. So the engine polls, in the way GitHub recommends when webhooks are not used, and on the interval the user fixed: every 3 minutes until everything is green, never longer.

- **Conditional requests are free while nothing changes.** Measured on PR #454 on 2026-10-04: an unconditional GET cost one rate-limit point, and three GETs with `If-None-Match` returned `304` and cost none. The same held for the PR's check runs, status and comments.
- **Detectors.** Per watched PR: the PR itself, the check runs and status of its head commit, its comments (where the verdict lands) and its review comments. Per repository with a queued PR: the base branch ref. Per epic: its sub-issues and open blockers.
- **Schedule.** One constant: 180 seconds, for every watched PR, from the moment it enters babysit until it is merged or closed. No backoff, no idle streak, no per-state interval. Today's workflow starts at 3 minutes and backs off; that backoff is removed. A worker report or a merge by the engine triggers an immediate extra check and never delays the next scheduled one. After an error the engine honours `retry-after` and otherwise tries again at the next 3-minute mark.
- **Cost.** At most 20 checks an hour per watched PR, far below GitHub's limits (5,000 points an hour, 900 points a minute per endpoint).
- **Tick.** Each check of a PR in babysit is one deterministic tick, which reads the full state with one GraphQL query. Green CI, zero unresolved threads and an approved zero-finding verdict move the issue to the merge queue with no LLM turn. A fix item, an actionable thread or an escalation prompts the worker. Outside epics, `toolu babysit wait` blocks the same way and returns only when there is something to act on.
- **Reaction time.** At most 3 minutes after the verdict, the same as the first tick of today's cron and better than its backed-off ticks. What changes is the cost: a check is a few HTTP requests in the engine, not an LLM turn in the worker's session.

What the `gh` CLI offers, investigated on 2026-10-04:

- `gh run watch` and `gh pr checks --watch` poll, every 3 and 10 seconds by default. Core `gh` has no event subscription.
- The `gh webhook forward` extension is the one way to receive webhooks with no tunnel and no App: it creates a repository hook named `cli` and receives deliveries over an outbound WebSocket to a GitHub relay. GitHub documents it as "only designed for use during testing and development" and "not supported for use in production". Only one forwarder may run per repository, so two machines on one repository cannot both use it. Its reconnect gives up after three attempts and has an open bug (cli/gh-webhook#43), the hook appears to stay on the repository after exit, and the relay protocol is undocumented.
- The Events API is documented as not real-time, with latency from 30 seconds to 6 hours.

Jev: 0.95 for polling over `gh webhook forward` in the first version. GitHub changes enter the state machine through one detector interface, so a push source such as `gh webhook forward` can be tried later without touching it.

### Merge queue with mechanical rebase

Jev: 1.00 for this over prompting each worker to rebase.

- One queue per repository, ordered by how much work a merge unblocks, then by time ready.
- Only the head is brought up to date. The engine fences the worktree (the worker is parked at `ready`), snapshots it, runs `git rebase origin/<base>` and pushes with `--force-with-lease`. A conflict aborts the rebase and prompts the worker with `REBASE` as today.
- After a clean rebase, CI and the review bot run on the new head. The engine arms GitHub auto-merge pinned to that SHA and learns of the merge from the PR detector.
- The invariant is unchanged: before every merge the head contains the base tip, every check passes and no thread is unresolved.

### Journal and state

Jev: 1.00 for a journal plus snapshots over SQLite or files only.

- An append-only JSON-lines journal per machine: every event, transition, gate verdict, prompt sent to a worker, judgment event delivered, launch, merge, job admission and wait, GitHub budget sample. Closed schema, sequence numbers, daily rotation, retention.
- Today's per-epic JSON records stay, written atomically, as snapshots that people and scripts can read. An epic started by the TypeScript watcher can be adopted by the engine.
- Leases, policy and pressure stay in the shared resource store with file locks, so job admission works with no engine running.

### HTTP status server

Jev: 0.99 for read-only plus a live stream and dashboard over a control API or bare JSON.

- `GET` only. JSON under `/api/v1/`: overview, epics, one epic (graph, waves, queue), issues, agents, hosts, machine, jobs, merge queue, attention, events (paged by sequence), metrics. `/api/v1/events/stream` is a server-sent event stream. `/` is one embedded HTML page with no external assets.
- **Tasks:** per issue, the stage, phase and its age, host, model, effort, PR, last gate verdict, queue position, launches, last events. **Hosts:** per worker host (Claude Code, Codex, Cursor Agent, OpenCode), the cap, running count, cooldown and reason; for the machine, load, available memory, pressure state, agent and job leases with owner and age.
- **Security:** binds `127.0.0.1` by default. Any other bind refuses to start without a bearer token. `Host` header allow-list against DNS rebinding, no CORS, `no-store`. No pane transcripts, briefs, environment or tokens in any response.
- `toolu epic status --json` returns the same documents over the engine's local socket, so the skill needs no HTTP.
- **Several machines.** Each machine runs its own engine. A later sub-issue lets one server list peers and merge their overviews into a fleet view (Jev: 0.29 that this is needed in the first version).

### Attention and metrics

- **Attention queue.** needs-human is 16% of wall time. Judgment events go to `toolu epic wait`, a herdr notification, an optional outgoing notification URL (Slack, ntfy) and the dashboard, each with its age. `toolu epic answer <key> "<text>"` relays the answer to the worker and journals it.
- **Metrics and budgets**, computed from the journal and gated where they can be:

  | Measure | Baseline | Target |
  |---|---|---|
  | First `ready` to merged, checks green, no conflict | p90 69 min (30 to 45 h in the outlier epic) | p90 4 min or less, excluding CI time |
  | Orchestrator LLM wake-ups with no judgment | every 5 min awaiting merge, every 45 min heartbeat | 0 |
  | Worker LLM turns on keep-going babysit ticks | one per tick | 0 |
  | Worker `REBASE` prompts | 119 for 83 issues | only on conflict |
  | Review finished to engine reaction | 3 min at first, longer once the cron backs off | 3 min or less, always |
  | Gap between babysit checks | 3 min, growing with backoff | 180 s, constant |
  | Resident supervisor | about 31 MB per epic | 10 MB or less for all epics |

## Alternatives

| Option | Verdict | Why |
|---|---|---|
| Port the watcher and keep the LLM as the control loop | Rejected | Keeps every measured wait and wake-up. Jev: 0.00. |
| Engine watches and serves status, LLM still triggers merges and launches | Rejected | Epics still stop when the LLM session does. Offered to the user, who chose the autonomous engine. |
| Engine merges, LLM approves every launch | Rejected | Slots sit idle while the LLM is away; the coupling check can be scored. |
| Keep prompting workers to rebase | Rejected | 119 LLM-driven rebase rounds. Jev: 0.00. |
| Merge without being up to date | Rejected | Breaks the merge invariant. |
| Stacked merge train (test several PRs together) | Deferred | Bigger gain per CI cycle, but needs failure bisection. Revisit with journal data. |
| GitHub webhooks through a tunnel, a GitHub App or a relay service | Rejected | User decision: no tunnel and no GitHub App. A webhook needs a way in to the machine. |
| `gh webhook forward` | Deferred | The only push `gh` offers without a tunnel, but documented as test-only, one forwarder per repository, an open reconnect bug. A possible later experiment. |
| Adaptive polling (faster while waiting, backing off when idle) | Rejected | User decision: the check interval is 3 minutes and never grows. |
| Fixed 3-minute checks with conditional requests | **Chosen** | Predictable, free while nothing changes, supported, no infrastructure. |
| SQLite for all state | Rejected | Loses the readable per-epic files and the TypeScript-compatible layout during migration. |
| HTTP control API in the first version | Deferred | A write surface on a network port needs its own threat model; control stays in the CLI. |

## Open risks

- **Unattended merges.** Mitigations: unchanged gate, merge pinned to the head SHA, persisted pause, journal of every action. The first epics on the engine should be watched through the dashboard.
- **Mechanical rebase in a worker's worktree.** It runs only under the worktree fence while the worker is parked, after a snapshot, and aborts on conflict.
- **Reaction time is bounded by the interval.** A green verdict is acted on up to 3 minutes after it lands. That is the chosen trade for a fixed, predictable check with no infrastructure.
- **These gains arrive with phase 5.** Phases 0 to 2 come first. If the waits hurt before then, the merge queue and auto-gate could be prototyped in the TypeScript watcher; that is not planned.
