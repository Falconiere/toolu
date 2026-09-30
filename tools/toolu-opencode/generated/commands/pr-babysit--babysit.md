---
name: pr-babysit--babysit
---
# Babysit a PR

Read `${TOOLU_PLUGIN_ROOT}/workflows/babysit.md` completely and execute its
Claude Code controller plus the shared strict-clearance workflow. Pass
`$ARGUMENTS` as the workflow input. Claude Code retains the cron-driven
continuation mechanism defined there.

Each tick is one command: `bun "${TOOLU_PLUGIN_ROOT}/hooks/dist/babysit-tick.js"`
with `--state-file /tmp/pr-babysit-<slot>.json`; its result contract is
`${TOOLU_PLUGIN_ROOT}/skills/babysit/references/helper.md`. Trust that
result — never write a polling script or controller of your own, and
never re-fetch with ad-hoc `gh` calls what it already reports — and act through
`babysit-reply-thread.js`, `babysit-resolve-thread.js` and `babysit-record.js`.
