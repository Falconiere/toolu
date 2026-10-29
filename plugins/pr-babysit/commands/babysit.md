# Babysit a PR

Read `${CLAUDE_PLUGIN_ROOT}/workflows/babysit.md` and its linked Claude Code
controller completely; load its fix procedure whenever directed. Execute the
controller and shared strict-clearance workflow. Pass
`$ARGUMENTS` as the workflow input. Between cycles, Claude Code runs
`toolu babysit wait` as a background Bash call, as defined there.

Every check is `toolu babysit tick` or `toolu babysit wait` with
`--state-file /tmp/pr-babysit-<slot>.json`; its result contract is
`${CLAUDE_PLUGIN_ROOT}/skills/babysit/references/helper.md`. Trust that
result — never write a polling script or controller of your own, and
never re-fetch with ad-hoc `gh` calls what it already reports — and act through
`toolu babysit reply`, `toolu babysit resolve` and `toolu babysit record`.
