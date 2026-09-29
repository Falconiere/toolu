---
name: epic-orchestrator--epic
---
# Epic orchestrator

Resolve the plugin root, then read and execute the skill:

```bash
ROOT="${TOOLU_PLUGIN_ROOT}"
ROOT="${ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT}}}"
```

Read `$ROOT/skills/epic-orchestrator/SKILL.md` completely and execute it. Pass
`$ARGUMENTS` as the epic reference (GitHub, Jira, or Linear) plus flags
(`--dry-run`, `--max N`, `--hosts claude:2,codex:2`, `--repo owner/name`,
`--tracker`, `--safe`, `--no-jev`, `status`, `stop`).

Scripts live under `$ROOT/scripts` and are all invoked with `bun`. Trust
script output — do not re-fetch with ad-hoc `gh` what the graph, gate, or
watcher already reported.
