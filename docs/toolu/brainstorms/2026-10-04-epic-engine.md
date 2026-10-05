# Epic engine: an efficient, autonomous epic-orchestrator with a status server

Brainstorm, 2026-10-04. Part of the Rust rebuild (epic #402). The user called epic-orchestrator the most valuable plugin, asked that it be efficient and effective, and asked for an HTTP server to consult the status of running tasks and hosts. A follow-up asked whether babysit can react the moment a code review finishes instead of polling.

## Capsule

- **Outcome:** `toolu epic` runs as a resident engine that drives an epic to done on its own. It gates, queues, rebases, merges, cleans up and launches the next issue without an LLM turn, and wakes the orchestrator LLM only for judgment. It reacts to events (herdr socket, worker reports, GitHub webhooks) instead of polling, keeps an append-only journal, and serves a read-only HTTP status API with a live dashboard.
- **Material defaults:**
  - Autonomous engine (user decision), inside today's authorization boundary.
  - Per-repository merge queue with mechanical rebase of the head PR only.
  - Babysit ticks run in the engine, triggered by GitHub events; the worker LLM is prompted only when there is something to fix.
  - GitHub webhooks arrive at one HMAC-verified receiver through a direct URL or a named tunnel. A slow conditional-request reconcile stays on for correctness.
  - Journal (JSON lines) plus today's JSON records as snapshots.
  - HTTP server: read-only, loopback by default, bearer token for any other bind.
- **Non-goal:** changing what the engine is allowed to do, relaxing the "head contains base tip" rule, a write API over HTTP, stacked merge trains, Jira/Linear webhooks.
- **Repository evidence:** 115 issue records in the real state directories under `~/.claude/epics/`, the watcher and skill source, herdr's socket API schema, GitHub's webhook documentation.
- **Risk:** an engine that merges unattended must fail closed and be easy to pause; webhook exposure needs a narrow, verified endpoint; mechanical rebase must never run while a worker is active in the worktree.
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

### Event sources instead of polling

| Source | Today | Engine |
|---|---|---|
| Agent state | `herdr agent list` every 60 s | herdr socket `events.subscribe` (`pane.agent_status_changed`, `pane.exited`, `pane.closed`, `worktree.removed`) |
| Worker phase | status file read every 60 s | `toolu epic report` sends to the engine's socket; the file is kept as a spool when the engine is down |
| CI, review, threads, merges, base moved | LLM cron tick every 3 min | GitHub webhooks |
| Issues closed or reopened outside the run, new sub-issues, dependencies | graph re-run every 45 min | `issues`, `sub_issues`, `issue_dependencies` webhooks |

### Babysit that reacts when the review finishes

Yes, this is possible. The review bot is a GitHub Action that posts its verdict as a PR comment, and GitHub sends webhooks for exactly that: `issue_comment` for the verdict, `check_run`, `check_suite` and `workflow_run` when jobs finish, `pull_request_review`, `pull_request_review_comment` and `pull_request_review_thread` for human review, `pull_request` for pushes and merges, `push` when the base moves. GitHub's own guidance is to subscribe to webhooks instead of polling.

- **Receiver.** The engine listens on `POST /hooks/github`, verifies `X-Hub-Signature-256`, drops duplicates by `X-GitHub-Delivery`, acknowledges within GitHub's 10-second limit and queues the event. This listener is separate from the status API, so a tunnel exposes only the hook path.
- **Transport.** Jev: 1.00 for a normal repository webhook to a stable URL, which is direct on a VPS and a named `cloudflared` tunnel on a machine behind NAT (`cloudflared` is already installed on the Mac). Several machines each register their own webhook. `gh webhook forward` needs no setup and is offered as a quick start, but GitHub documents it as unsupported for production and allows one forwarder per repository. A custom relay service was rejected as extra infrastructure to run.
- **Correctness.** GitHub does not redeliver failed deliveries on its own. The engine reconciles on start, on reconnect and every 10 minutes: it asks GitHub for failed deliveries and redelivers them, and it re-reads each open PR with conditional requests, which cost no rate limit when nothing changed. With no webhook configured, the same conditional polling runs on an adaptive interval, so the engine works everywhere and is faster with push.
- **Tick.** An event for a PR triggers one deterministic tick. Green CI, zero unresolved threads and an approved zero-finding verdict move the issue to the merge queue with no LLM turn. A fix item, an actionable thread or an escalation prompts the worker. Outside epics, `toolu babysit wait` blocks the same way and returns only when there is something to act on.

### Merge queue with mechanical rebase

Jev: 1.00 for this over prompting each worker to rebase.

- One queue per repository, ordered by how much work a merge unblocks, then by time ready.
- Only the head is brought up to date. The engine fences the worktree (the worker is parked at `ready`), snapshots it, runs `git rebase origin/<base>` and pushes with `--force-with-lease`. A conflict aborts the rebase and prompts the worker with `REBASE` as today.
- After a clean rebase, CI and the review bot run on the new head. The engine arms GitHub auto-merge pinned to that SHA and learns of the merge from the `pull_request` event.
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

- **Attention queue.** needs-human is 16% of wall time. Judgment events go to `toolu epic wait`, a herdr notification, an optional outgoing webhook (Slack, ntfy) and the dashboard, each with its age. `toolu epic answer <key> "<text>"` relays the answer to the worker and journals it.
- **Metrics and budgets**, computed from the journal and gated where they can be:

  | Measure | Baseline | Target |
  |---|---|---|
  | First `ready` to merged, checks green, no conflict | p90 69 min (30 to 45 h in the outlier epic) | p90 2 min or less, excluding CI time |
  | Orchestrator LLM wake-ups with no judgment | every 5 min awaiting merge, every 45 min heartbeat | 0 |
  | Worker LLM turns on keep-going babysit ticks | one per tick | 0 |
  | Worker `REBASE` prompts | 119 for 83 issues | only on conflict |
  | Review finished to engine reaction | up to the tick interval (3 min base) | 10 s or less with a webhook |
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
| `gh webhook forward` as the default transport | Rejected | Documented as test-only; one forwarder per repository. Kept as a quick start. |
| Own relay service for webhooks | Rejected | Extra infrastructure; a tunnel or a direct URL does the job. |
| Polling only, with conditional requests | Kept as fallback | Free when nothing changed, but slower than push. |
| SQLite for all state | Rejected | Loses the readable per-epic files and the TypeScript-compatible layout during migration. |
| HTTP control API in the first version | Deferred | A write surface on a network port needs its own threat model; control stays in the CLI. |

## Open risks

- **Unattended merges.** Mitigations: unchanged gate, merge pinned to the head SHA, persisted pause, journal of every action. The first epics on the engine should be watched through the dashboard.
- **Mechanical rebase in a worker's worktree.** It runs only under the worktree fence while the worker is parked, after a snapshot, and aborts on conflict.
- **Webhook endpoint.** Only the hook path is exposed, every delivery is signature-checked, and the secret is per repository in a 0600 file.
- **These gains arrive with phase 5.** Phases 0 to 2 come first. If the waits hurt before then, the merge queue and auto-gate could be prototyped in the TypeScript watcher; that is not planned.
