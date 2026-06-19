# dashboard-config

Read or edit the machine config for the toolu execution dashboard
(`${XDG_CONFIG_HOME:-~/.config}/toolu/dashboard.json`). The dashboard scans the
configured `roots` for every project's plan-ledger and shows each one's plan lane
plus its live agent-activity tree.

Run the CLI from the repo root via Bun:

```bash
# show the current effective config (defaults filled in)
bun run plugins/toolu/dashboard/config-cli.ts get

# add / remove a base dir to scan (one repo or a parent of many worktrees)
bun run plugins/toolu/dashboard/config-cli.ts add-root ~/Projects
bun run plugins/toolu/dashboard/config-cli.ts add-root ~/.herdr/worktrees
bun run plugins/toolu/dashboard/config-cli.ts rm-root ~/Projects

# tune a scalar (scanDepth, activeWithinHours, stuckThresholdSeconds,
# agentStuckSeconds, pollMs, port, open)
bun run plugins/toolu/dashboard/config-cli.ts set pollMs 1000
bun run plugins/toolu/dashboard/config-cli.ts set activeWithinHours 24
```

Then launch the dashboard:

```bash
bun run plugins/toolu/dashboard/index.ts --open
```

When the user asks to configure or set up the dashboard, run the appropriate
`config-cli.ts` subcommand above (resolving `~`/relative paths to the dir they
mean), confirm the resulting `roots`, and offer to launch the dashboard.
