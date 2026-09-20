# Babysit a PR

Read `${CLAUDE_PLUGIN_ROOT}/workflows/babysit.md` completely and execute its
Claude Code controller plus the shared strict-clearance workflow. Pass
`$ARGUMENTS` as the workflow input. Claude Code retains the cron-driven
continuation mechanism defined there.

Each tick is one command: `bash "${CLAUDE_PLUGIN_ROOT}/scripts/babysit-tick.sh"`
with `--state-file /tmp/pr-babysit-<slot>.json`; its result contract is
`${CLAUDE_PLUGIN_ROOT}/skills/babysit/references/helper.md`. Trust that
result — never write a polling script or controller of your own, and
never re-fetch with ad-hoc `gh` calls what it already reports — and act through
`reply-thread.sh`, `resolve-thread.sh` and `record.sh`.
