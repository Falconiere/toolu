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

The skill passes `--safe` and `--no-jev` to `toolu epic start <state_dir>`,
which records them in `<state_dir>/launch.json` for the engine's launches,
and `--no-jev` also to `toolu epic route` before the start. `--kind`,
`--model` and `--effort` go to a manual `toolu epic launch`.

For `status`, use `toolu --json epic status` and show its versioned engine
overview; `toolu --json epic status <key>` selects one epic or worker. The
read-only dashboard is at `http://127.0.0.1:7717/` while the engine runs.

Every action is a `toolu epic` verb (see `docs/cli/epic.md`). Trust its
output — do not re-fetch with ad-hoc `gh` what the graph, gate, or engine
already reported.
