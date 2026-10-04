---
description: "Use when the user asks for current repository, branch, working-tree, quality-gate, comemory, Jev readiness, or toolu plugin readiness status in OpenCode."
name: "statusline-status"
---

# Status

Run this through bash and return its output verbatim:

```bash
# OpenCode
"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"
```

`shell.env` sets `TOOLU_BUN`, `TOOLU_PLUGIN_ROOT_STATUSLINE`, `TOOLU_CONFIG_DIR` and
`TOOLU_HOST_OVERRIDE=opencode` in every bash call. The report starts with toolu's
readiness from this project's startup record: the plugins that started, with
their startup entries, the selection source and any startup notes. A
`toolu: no startup record` or `toolu: unreadable startup record` line names its
next step; repeat that step to the user. OpenCode has no persistent statusline,
so this report is the status surface: the Claude Code statusline and its setup
command do not apply. The report includes only fields available from local
repository and toolu state. Do not invent Claude-only account, model, effort,
or context-window values when the active host does not expose them.
