# epic-orchestrator — Epic Orchestrator

Drive a GitHub, Jira, or Linear epic to merged PRs: dependency graph, herdr
worktrees, parallel workers on Claude Code, Codex, Cursor Agent, or Muse Code
(delivery-flow from brainstorm through PR and babysit) routed by Jev
complexity, a merge gate with GitHub auto-merge, rate-limit and progress
guardrails, and cleanup. See the plugin README for trackers, hosts, and
guardrails.

## Install

```text
/plugin install epic-orchestrator@toolu
```

```bash
toolu plugins install delivery-flow epic-orchestrator --host codex
toolu plugins install delivery-flow epic-orchestrator --host cursor
toolu plugins install delivery-flow epic-orchestrator --host muse --approve
```

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies. Runtime binaries: `toolu`, `gh`, `herdr`
(orchestrator session must run inside a herdr pane with `HERDR_ENV=1`).

## Usage

```bash
# Dry-run: show waves, blockers, and the first launch batch
# (or ask the agent: "dry-run epic owner/repo#N")

# Read the resident engine's versioned overview while a run is live
toolu epic status --json
# Start a foreground engine and local dashboard when no engine is running
# (default: http://127.0.0.1:7717/)
toolu epic serve
# Disable HTTP while keeping the control socket
toolu epic engine --no-http
# Register an epic and start the engine; wait for its next judgment
# (Claude Code: a background call, 45-minute default limit;
# Codex, Cursor Agent and Muse Code: a loop). Run these outside the host's
# sandbox: Codex's and Muse's block the engine socket with
# "Operation not permitted".
toolu epic start <state_dir>
toolu --json epic wait --epic <epic-key> --max-seconds 480
```

Every action is a `toolu epic` verb (reference: [`docs/cli/epic.md`](../cli/epic.md)):

| Verb      | Role                                                                                         |
| --------- | -------------------------------------------------------------------------------------------- |
| `graph`   | Build graph (GitHub, Jira, or Linear), classify issues, pick launch batch                    |
| `route`   | Jev complexity tier, then host, model, and effort per issue                                  |
| `launch`  | Clone/fetch, herdr worktree + routed host agent, worker brief                                |
| `gate`    | Assess/merge PR; `--auto` arms GitHub auto-merge; `--admin` only for protection-only blocks  |
| `finish`  | Snapshot, then tear down agent/worktree after merge; a live engine is told, never relaunches |
| `close`   | Post the summary and close the epic in its tracker                                           |
| `release` | Release a coupling hold so the issue launches next                                           |
| `report`  | Worker phase reporter                                                                        |
| `wait`    | The orchestrator's next judgment from the resident engine                                    |
| `metrics` | Epic runs measured from the engine journal, beside the TypeScript baseline                   |
| `summary` | One epic's report: measures, budgets and token use per worker                                |
| `baseline`| Derive the TypeScript-era baseline from recorded epic state directories                      |

Full procedure: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.

## Finish recovery

When the worker's agent and pane are already gone and the PR is merged,
`toolu epic finish <state_dir> <key>` treats the shutdown as done: it
snapshots when the worktree still exists, removes what remains, and releases
the lease, so just retry the finish.

## Engine events

The resident engine subscribes to herdr (`events.subscribe`) instead of polling.
herdr answers one request per socket connection, so each herdr call, the
session's ping and snapshot included, opens its own connection; only the
subscription stays open.
The engine starts its dashboard at `http://127.0.0.1:7717/` by default.
It shows registered epics,
workers, hosts, jobs, machine capacity, queue and attention, and follows the
journal without polling. `toolu epic status --json <key>` returns
one epic or worker through the local control socket.
Selecting a worker opens its drawer: plan ledger steps, agent status, phase
history and Screen and Logs tabs. With the token, Screen reads the agent's live
pane every 2 seconds while selected and visible; a blocked agent shows the
`herdr agent attach` command to copy. Logs lists this worker's managed jobs and
Claude Code task output, follows the selected log every second while selected
and visible, and offers a jump to the latest output after scrolling up. The
list distinguishes running and done sources and reports empty or unreadable
logs. `#issue=<key>` links to a drawer. Logs are local to the serving machine;
the peer dashboard link opens that machine's logs.

### Dashboard control

The dashboard can also control the engine that serves it: pause or resume all
epics, one epic or a host, answer a worker, release a coupling hold and
acknowledge attention. Peers stay read-only.

To unlock it, run `toolu epic token url` and open the URL it prints. The token
travels in the `#token=` fragment, so it never reaches a server log; the page
keeps it in this tab's `sessionStorage` and removes it from the address bar, so
a copied dashboard URL holds no token. Treat the token URL like a password:
anyone who opens it can type into agents that run with approval bypass.
`[ LOCK ]` forgets the token in this tab. After `toolu epic token new`, the next
action gets `401`, nothing changes on the engine, and the page shows
`● 401 · TOKEN ROTATED` until you open the new URL.

Every action needs the token, even on loopback, and is journaled with `by=web`;
the page shows the result and then the change as the journal stream arrives.
`ANSWER` reports `✓ DELIVERED`, `● REFUSED · <code>` or `▲ UNCERTAIN`; an
uncertain answer may still reach the worker, so check its screen before sending
again.

## Hosts

`toolu epic hosts pause <host>` takes one host (`claude`, `codex`, `cursor` or
`muse`) out of rotation, and `toolu epic hosts resume <host>` brings it back;
the dashboard does the same through `POST /api/v1/actions/host-pause` and
`host-resume`. The pause lives in the shared resource store, so it is
machine-wide and survives an engine restart. New agents route to the other
hosts; workers already running keep running, and a relaunch that would return
to the paused host waits with the reason `host paused`. Resuming also clears
the host's usage-limit cooldown, in the shared store and in every epic's
`hosts.json`, so an operator who knows a limit has reset can put the host back
at once. `/api/v1/hosts` reports `paused`, `paused_by` and `paused_at` for each
host. A manual `toolu epic launch --kind <paused host>` is refused unless
`--force` is passed.

`toolu epic token url` prints the running engine's dashboard URL with the token
in its `#token=` fragment. `toolu epic token new` rotates the token without a
restart.

Peers stay read-only. The versioned HTTP contract, the action routes and the
token rules are in [`docs/epic-status-api.md`](../epic-status-api.md).
`toolu epic report` writes the status file and, when the engine is down, a spool
the next start applies in order. Stall deadlines belong to each issue: 45
minutes, or 120 minutes while the phase is babysit. The 30-second tick is for
checkpoints, not for herdr.

Resource leases use a `state.lock` directory under the resource home. If an
external command creates a regular file there, lease commands report its path
and ask you to confirm no resource update is using it, then remove the file.
Inspect the named file before removal; toolu leaves its contents untouched.

## Metrics and budgets

`toolu epic metrics` (narrowed with `--epic` and `--since`) folds the engine journal
into lead time, phase shares, first `ready` to merged (split into queue, rebase,
CI, merge and rework), rebase rounds by engine or worker, worker prompts by
reason, judgments and their waits, orchestrator wake-ups, GitHub requests
(the share answered `304`, points, the interval each check scheduled next),
job admission waits and pressure holds, relaunches, host switches, cooldowns,
and the engine's peak memory and CPU time. The text form sets each measure
beside the TypeScript baseline; `--json` prints the document, which
`/api/v1/metrics` also serves (`$defs/epicMetrics` in `toolu commands
--schema`). A measure with no records is `null`; nothing is estimated.

| Budget | Limit | Held by |
|---|---|---|
| `judgment-free-wakeups` | 0 `wait` replies without a judgment | scripted epic in CI |
| `prompts-while-waiting` | 0 worker prompts without a babysit finding | scripted epic in CI |
| `rebase-without-conflict` | 0 `REBASE` prompts without a conflict hand-off | scripted epic in CI |
| `verdict-reaction` | 0 green verdicts that waited for another check | scripted epic in CI |
| `check-gap` | each check schedules the next at most 180 s out | scripted epic in CI; it catches a changed interval or a backoff, not an observed delay |
| `ready-to-merged` | p90 at most 240 s from first `ready` to merged, CI excluded, issues without rework | scripted epic in CI |
| `idle-engine` | 10 MB resident, no timer under 30 s | `crates/epic-orchestrator/tests/idle_budget.rs` on Linux |

Real runs report the same table: `toolu epic summary <epic>` prints it with the
measures beside the baseline and token use per worker (Claude Code
transcripts, Codex rollouts and Muse session logs; OpenCode and Cursor rows say
`unavailable`).
The engine appends the same section, without token rows, to the epic's
closing comment.

The baseline is `benchmarks/epic-baseline.json`, derived from the TypeScript
watcher's state directories as of 2026-10-04:

```sh
toolu epic baseline --state ~/.claude/epics --until 2026-10-05T01:30:00Z --outlier falconiere-toolu-orm-145
```

