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
| `cleanup` | Unregister locally finished epics or missing state directories; preserve files and Git state |
| `close`   | Post the summary and close the epic in its tracker                                           |
| `release` | Release a coupling hold so the issue launches next                                           |
| `report`  | Worker phase reporter                                                                        |
| `wait`    | The orchestrator's next judgment from the resident engine                                    |
| `metrics` | Epic runs measured from the engine journal, beside the TypeScript baseline                   |
| `summary` | One epic's report: measures, budgets and token use per worker                                |
| `baseline`| Derive the TypeScript-era baseline from recorded epic state directories                      |

Full procedure: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.

## Uncertain merges

The merge queue sends one PUT pinned to the verified PR head. If GitHub times
out, drops the response or answers 5xx, the engine holds that queue head and
raises `merge uncertain` attention. The PUT may have applied; inspect the PR on
GitHub before taking manual action. A later observation that the PR merged
releases the hold and starts cleanup. The hold survives an engine restart.
The engine does not resend an uncertain PUT or try the admin merge path.
Definite policy, authentication and validation refusals remain refusals. An
invalid URL, or a DNS or connection failure known to happen before the PUT is
sent, is reported as a local failure. The admin path
is available only for a protection refusal.

## Uncertain rebase pushes

The engine rebases a parked worker's branch and pushes it with
`--force-with-lease`. If the push reports an error, it reads the exact remote
branch once. A remote SHA equal to the rebased local HEAD confirms success.
An explicit Git rejection with a different readable remote SHA, or a failed
local pre-push hook, hands the rebase to the worker; the engine never
overwrites the other writer's ref. If the push may have reached the remote
but the remote cannot be read or the result is inconclusive, the engine holds
the queue head and raises `rebase push uncertain` attention. It does not push
again or send a worker `REBASE` prompt. Inspect the remote branch and PR head
before taking manual action; the hold survives an engine restart.

## Tracker close recovery

Closing a GitHub, Jira, or Linear issue or supported epic writes its delivery
comment intent under `close-intents/` in the epic state directory before
posting. A manual gate without `--state-dir` uses the local resource root.
Before any new POST, toolu checks the tracker's comments for the original text.
The receipt keeps that text if a later summary changes. If the comment landed
but the close or Done transition failed, rerun `toolu epic close` or the merged
issue's gate using the same state directory to finish the transition without
another comment.
A recovered resident close tries this safe path once. A persistent failure
remains blocked for explicit action.
An incomplete comment listing or unreadable receipt stops the close; it does
not authorize a second POST. Already-done items receive no delivery comment.

## Registry cleanup

Use `toolu epic cleanup --dry-run` to preview stale registry entries, then
`toolu epic cleanup` to unregister them. Both return JSON with `dry_run`,
`removed` (proposed removals in a preview), and `retained`; each item has its
original registry `entry` and a `reason`.

Cleanup uses local completion, closure and directory evidence, with no age
threshold or tracker requests. Active workers, pending actions, unfinished
teardown, failed or uncertain closure, and unreadable or malformed state keep
an entry registered. State files, journal history, worktrees, branches and
tracker issues stay in place. Repeating cleanup is harmless. It runs through
the resident engine when one owns the state, or takes its ownership lock
without starting workers. It affects this machine only; run it separately on
peers. To remove abandoned unfinished work, finish it explicitly first.

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
The local control socket is mode `0600`. A connected peer has five seconds
total to finish one JSON request line (up to 16 MiB); idle, incomplete and
slow-trickling requests close at that deadline. Once a valid request is read,
its reply is not subject to the read deadline, so `wait` and `replace` may
finish later. Stopping the engine joins incomplete request readers; accepted
reply workers keep their existing lifetime. [Release resource
measurements](evidence/control-request-deadline.md) show the before/after
thread and descriptor counts for stalled peers.
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

`toolu epic notify` also spools a worker notice. The engine writes its token to
the durable notify journal before recording the spool marker and deleting the
file. If it restarts between those writes, it recognizes that token across the
retained journal and does not raise attention or relay to a sibling again.
An already recorded token also discards a stale spool file on a live retry.
Distinct tokens remain distinct notices; the existing recipient cooldown still
limits relay frequency. A crash after journaling but before the herdr prompt
can leave the sibling without that prompt. A prompt whose result is uncertain
cannot be proved delivered exactly once without a receiver acknowledgment.

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

