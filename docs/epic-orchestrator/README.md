# epic-orchestrator — Epic Orchestrator

Drive a GitHub, Jira, or Linear epic to merged PRs: dependency graph, herdr
worktrees, parallel workers on Claude Code, Codex, Cursor Agent, or OpenCode
(delivery-flow from brainstorm through PR and babysit) routed by Jev
complexity, a merge gate with GitHub auto-merge, rate-limit and progress
guardrails, and cleanup. See the plugin README for trackers, hosts, and
guardrails.

## Install

```text
/plugin install epic-orchestrator@toolu
```

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies. Runtime binaries: `toolu`, `gh`, `herdr`
(orchestrator session must run inside a herdr pane with `HERDR_ENV=1`).
OpenCode workers also need `opencode` 1.x on `PATH`; see the OpenCode section
of the plugin README.

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
toolu epic start <state_dir>
toolu --json epic wait --max-seconds 480
```

Every action is a `toolu epic` verb (reference: [`docs/cli/epic.md`](../cli/epic.md)):

| Verb      | Role                                                                                         |
| --------- | -------------------------------------------------------------------------------------------- |
| `graph`   | Build graph (GitHub, Jira, or Linear), classify issues, pick launch batch                    |
| `route`   | Jev complexity tier, then host, model, and effort per issue                                  |
| `launch`  | Clone/fetch, herdr worktree + routed host agent, worker brief                                |
| `gate`    | Assess/merge PR; `--auto` arms GitHub auto-merge; `--admin` only for protection-only blocks  |
| `finish`  | Snapshot, then tear down agent/worktree after merge                                          |
| `close`   | Post the summary and close the epic in its tracker                                           |
| `release` | Release a coupling hold so the issue launches next                                           |
| `report`  | Worker phase reporter                                                                        |
| `wait`    | The orchestrator's next judgment from the resident engine                                    |

Full procedure: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.

## Engine events

The resident engine subscribes to herdr (`events.subscribe`) instead of polling.
The engine starts its read-only dashboard at `http://127.0.0.1:7717/` by default.
It shows registered epics,
workers, hosts, jobs, machine capacity, queue and attention, and follows the
journal without polling. `toolu epic status --json <key>` returns
one epic or worker through the local control socket. The versioned HTTP
contract and token rules are in [`docs/epic-status-api.md`](../epic-status-api.md).
`toolu epic report` writes the status file and, when the engine is down, a spool
the next start applies in order. Stall deadlines belong to each issue: 45
minutes, or 120 minutes while the phase is babysit. The 30-second tick is for
checkpoints, not for herdr.
