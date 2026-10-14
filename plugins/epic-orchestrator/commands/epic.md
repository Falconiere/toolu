# Epic orchestrator

`$ARGUMENTS` is a subcommand below, or an epic reference (GitHub, Jira, or
Linear) with the skill's flags (`--dry-run`, `--max N`, `--hosts
claude:2,codex:2`, `--repo owner/name`, `--tracker`, `--safe`, `--no-jev`).

An epic reference runs the `epic-orchestrator` skill: load it (on Claude Code
`ROOT="${CLAUDE_PLUGIN_ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT}}}"`, then
`$ROOT/skills/epic-orchestrator/SKILL.md`), read it completely and execute it.

Each subcommand is one CLI command; print its output and stop. `<epic-key>`
is the basename of the epic's state directory (for example
`falconiere-toolu-402`); `<state_dir>` is the directory itself.

| Subcommand | Runs |
|---|---|
| `start <state_dir>` | `toolu epic start <state_dir>` (add `--safe`, `--no-jev` when passed), then the skill's wait loop |
| `status [<epic-key>]` | `toolu epic status <epic-key>` (all epics without a key), plus the dashboard `http://127.0.0.1:7717/`; consumes no judgment |
| `fleet` | `toolu epic status --fleet`: this machine and every peer engine in `epic.peers`, with each peer's state; consumes no judgment |
| `pause [<epic-key>]` | `toolu epic pause <epic-key>`: no launch, rebase or merge until resumed |
| `resume [<epic-key>]` | `toolu epic resume <epic-key>` |
| `stop <epic-key>` | `toolu epic pause <epic-key>`, then end the wait loop; workers finish their step and go idle |
| `serve` | `toolu epic serve`: a foreground engine with its dashboard, when none runs |

Every action is a `toolu epic` verb (see `docs/cli/epic.md`). Trust its
output; do not re-fetch with `gh` what the engine already reported.
