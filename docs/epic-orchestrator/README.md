# epic-orchestrator — Epic Orchestrator

Drive a GitHub epic to merged PRs: dependency graph, herdr worktrees, parallel
Claude workers (delivery-flow from brainstorm through PR and babysit), merge gate, and cleanup.

## Install

```text
/plugin install epic-orchestrator@toolu
```

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

Requires `delivery-flow` and its `toolu`, `toolu-review`, and `pr-babysit` dependencies. Runtime binaries: `bun`, `gh`, `herdr`
(orchestrator session must run inside a herdr pane with `HERDR_ENV=1`).

## Usage

```bash
# Dry-run: show waves, blockers, and the first launch batch
# (or ask the agent: "dry-run epic owner/repo#N")

# Status / stop while a run is live
# status — graph table + watcher peek
# stop — stop the background watcher only
```

Scripts (invoked by the skill via `${CLAUDE_PLUGIN_ROOT}/scripts`):

| Script | Role |
|--------|------|
| `epic-graph.ts` | Build graph, classify issues, pick launch batch |
| `launch-issue.ts` | Clone/fetch, herdr worktree + agent, worker brief |
| `epic-watch.ts` | Background poll; emit ready/failed/blocked/… events |
| `merge-gate.ts` | Assess/merge PR; `--admin` only for protection-only blocks |
| `finish_issue.sh` | Tear down agent/worktree after merge |
| `report.sh` | Worker phase reporter |

Full procedure: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.
