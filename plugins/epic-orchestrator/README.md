# epic-orchestrator

Drive a GitHub epic to merged PRs. Builds the sub-issue dependency graph,
launches herdr worktrees with Claude workers (spec → plan → execution →
pr-babysit), merges green PRs, and cleans up until the epic is complete.

## Install

```text
/plugin install epic-orchestrator@toolu
```

```bash
codex plugin add toolu@toolu
codex plugin add pr-babysit@toolu
codex plugin add epic-orchestrator@toolu
```

Requires the `toolu` and `pr-babysit` plugins. Runtime: `bun`, `gh`, and `herdr`
(with `HERDR_ENV=1` inside a herdr pane).

## What it provides

- **Claude `/epic-orchestrator:epic` and skill `epic-orchestrator`** — orchestrate
  an epic (or `status` / `stop`). Workers run the toolu delivery chain then
  babysit; the orchestrator merges and advances the graph.
- **Bun CLIs** under `scripts/` — `epic-graph.ts`, `launch-issue.ts`,
  `epic-watch.ts`, `merge-gate.ts`, plus `finish_issue.sh` / `report.sh`.
- **Codex SessionStart** — warns when `toolu` or `pr-babysit` is missing.
