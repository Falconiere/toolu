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

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies. Runtime binaries: `bun`, `gh`, `herdr`
(orchestrator session must run inside a herdr pane with `HERDR_ENV=1`).
OpenCode workers also need `opencode` 1.x on `PATH`; see the OpenCode section
of the plugin README.

## Usage

```bash
# Dry-run: show waves, blockers, and the first launch batch
# (or ask the agent: "dry-run epic owner/repo#N")

# Status / stop while a run is live
# status — graph table + watcher peek
# stop — stop the background watcher only
```

Scripts (invoked by the skill via `${CLAUDE_PLUGIN_ROOT}/scripts`):

| Script            | Role                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------- |
| `epic-graph.ts`   | Build graph (GitHub, Jira, or Linear), classify issues, pick launch batch                   |
| `route.ts`        | Jev complexity tier, then host, model, and effort per issue                                 |
| `launch-issue.ts` | Clone/fetch, herdr worktree + routed host agent, worker brief                               |
| `epic-watch.ts`   | Background poll; ready/failed/blocked/host-limited/gh-budget-low events; checkpoints        |
| `merge-gate.ts`   | Assess/merge PR; `--auto` arms GitHub auto-merge; `--admin` only for protection-only blocks |
| `checkpoint.ts`   | Snapshot a worktree to `refs/epic-wip/<key>`                                                |
| `epic-close.ts`   | Post the summary and close the epic in its tracker                                          |
| `finish-issue.ts` | Snapshot, then tear down agent/worktree after merge                                         |
| `report.ts`       | Worker phase reporter                                                                       |

Full procedure: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.

## Engine events

The resident engine subscribes to herdr (`events.subscribe`) instead of polling.
`toolu epic report` writes the status file and, when the engine is down, a spool
the next start applies in order. Stall deadlines belong to each issue: 45
minutes, or 120 minutes while the phase is babysit. The 30-second tick is for
checkpoints, not for herdr.
