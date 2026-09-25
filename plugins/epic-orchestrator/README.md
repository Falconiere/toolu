# epic-orchestrator

Drive a GitHub epic to merged PRs. Builds the sub-issue dependency graph,
launches herdr worktrees with workers (delivery-flow from brainstorm through PR and babysit), merges green PRs, and cleans up until the epic is complete.

Works on **Claude Code**, **Codex**, **Cursor Agent**, and **OpenCode**. The
orchestrator skill runs on whichever host you invoke it from; workers still
default to `herdr agent start --kind claude` (override with `--kind`).

## Install

Requires `delivery-flow` and its `toolu`, `toolu-review`, and `pr-babysit` dependencies. Runtime: `bun`, `gh`, and `herdr`
(with `HERDR_ENV=1` inside a herdr pane).

### Claude Code

```text
/plugin install epic-orchestrator@toolu
```

### Codex

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

### Cursor Agent

Install the toolu marketplace plugin the same way as Claude Code (Cursor loads
the Claude-shaped plugin tree). Then invoke the `epic-orchestrator` skill or
the `/epic-orchestrator:epic` command.

### OpenCode

```bash
opencode plugin add @toolu/opencode
```

Enable the bash plugins in `<project>/.opencode/toolu/plugins.json`:

```json
{ "version": 1, "enabled": ["delivery-flow", "epic-orchestrator"] }
```

Wire OpenCode to the generated surface under
`tools/toolu-opencode/generated/` (shipped in `@toolu/opencode`); see
[docs/opencode.md](../../docs/opencode.md). Set `TOOLU_PLUGIN_ROOT` to the
installed `plugins/epic-orchestrator` directory when running scripts from the
generated skill.

## What it provides

- **Skill `epic-orchestrator` and command `epic`** — orchestrate an epic (or
  `status` / `stop`). Workers run the toolu delivery chain then babysit; the
  orchestrator merges and advances the graph.
- **Bun CLIs** under `scripts/` — `epic-graph.ts`, `launch-issue.ts`,
  `epic-watch.ts`, `merge-gate.ts`, plus `finish_issue.sh` / `report.sh`.
- **Codex SessionStart** — warns when `delivery-flow` or its dependencies are missing.

## State directory

Override with `EPIC_STATE_HOME`. Otherwise:

| Host | Default |
|------|---------|
| Claude Code / Cursor Agent | `~/.claude/epics/<owner>-<repo>-<n>/` |
| Codex | `$CODEX_HOME/toolu/epics/…` (default `~/.codex/toolu/epics/…`) |
| OpenCode | `$TOOLU_OPENCODE_HOME/toolu/epics/…` (or `$OPENCODE_HOME/…`) |
