# Epic orchestrator

Resolve the plugin root, then read and execute the skill:

```bash
ROOT="${CLAUDE_PLUGIN_ROOT}"
ROOT="${ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT}}}"
```

Read `$ROOT/skills/epic-orchestrator/SKILL.md` completely and execute it. Pass
`$ARGUMENTS` as the epic reference (GitHub, Jira, or Linear) plus flags
(`--dry-run`, `--max N`, `--hosts claude:2,codex:2`, `--repo owner/name`,
`--tracker`, `--safe`, `--no-jev`, `status`, `stop`).

For `status`, use `toolu epic status --json` and show its versioned engine
overview; `toolu epic status --json <key>` selects one epic or worker. The
read-only dashboard is at `http://127.0.0.1:7717/` while the engine runs.

Scripts live under `$ROOT/scripts` and are all invoked with `bun`. Trust
script output — do not re-fetch with ad-hoc `gh` what the graph, gate, or
watcher already reported.
