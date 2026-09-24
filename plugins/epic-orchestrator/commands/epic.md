# Epic orchestrator

Read `${CLAUDE_PLUGIN_ROOT}/skills/epic-orchestrator/SKILL.md` completely and
execute it. Pass `$ARGUMENTS` as the epic reference plus flags (`--dry-run`,
`--max N`, `status`, `stop`).

Scripts live under `${CLAUDE_PLUGIN_ROOT}/scripts` and are invoked with `bun`
(for `*.ts`) or `bash` (for `finish_issue.sh` / `report.sh`). Trust script
output — do not re-fetch with ad-hoc `gh` what the graph, gate, or watcher
already reported.
