# @toolu/opencode

The [toolu](https://github.com/Falconiere/toolu) adapter for [OpenCode](https://opencode.ai). It runs toolu's TypeScript dispatcher in `tool.execute.before` and `tool.execute.after`, so an edit that violates a gate is denied before bytes change.

The adapter uses the documented plugin API (`opencode-ai@1.18.34`, `@opencode-ai/plugin@1.18.34`) pinned in [docs/opencode-host-contract.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode-host-contract.md). [#336](https://github.com/Falconiere/toolu/issues/336) established the native plugin entrypoint.

## Generated OpenCode surface

`bun run generate:opencode-surface` builds `generated/` from all 16 plugin manifests. The catalog records 18 skills, five subagents, four commands, three plugins with no Markdown surface, and the statusline setup command excluded as host-specific: OpenCode has no `statusLine` setting, so [OP-25](https://github.com/Falconiere/toolu/issues/359) ships the `statusline-status` skill instead. `bun run check:opencode-surface` detects drift. `bun run probe:opencode-surface` loads the generated Markdown in an isolated pinned OpenCode 1.18.34 profile and checks discovery and parsed agent/command configuration.

Skill names are unique, at most 64 characters, and use lowercase letters, digits, and single hyphens. The generator preserves YAML metadata and agent permissions, maps Claude tool lists to restrictive OpenCode permissions, and lets agents inherit the configured host model instead of emitting Claude model aliases. Local linked resources are copied into `generated/`; known skill references and source paths point to generated IDs and files.

The generated catalog stays inside the package. When toolu is ready, its config hook (the plugin's `config` callback, [#345](https://github.com/Falconiere/toolu/issues/345)) adds the selected plugins' surfaces to the host's config. OpenCode then discovers them natively, and nothing is copied into `.opencode/` or `~/.config/opencode/`:

- each skill's directory goes into `skills.paths`, so the native `skill` tool lists and loads it;
- agents and commands go in as config entries.

Your own definitions win:

- a skill with the same name anywhere OpenCode looks keeps your copy;
- your `agent.<id>` or `command.<id>` keys override toolu's, key by key.

## Install

```bash
npx @toolu/plugins install --host opencode
```

The [`@toolu/plugins`](https://www.npmjs.com/package/@toolu/plugins) CLI adds this package to the `plugin` array of OpenCode's global config (`--scope project` for the project's), keeping your comments and other entries. Name plugins (`install ts-quality --host opencode`) to write a selection instead of enabling everything. You can also write the selection yourself, in your project:

```jsonc
// .opencode/toolu/plugins.json
{ "version": 1, "enabled": ["toolu"] }
```

Selection works in three tiers:

- **Project:** the file above. When present, it alone decides.
- **Global:** without a project file, `~/.config/opencode/toolu/plugins.json` (same schema) applies to every project.
- **Default:** with neither file, every bundled plugin is enabled.

Dependencies are added automatically. An invalid selection file stops toolu: every tool call is refused until you fix it.

Restart OpenCode. The package carries plugin manifests, settings and committed Bun bundles, so there is no clone and no `TOOLU_REPO_ROOT` to export. Update with `npx @toolu/plugins update --host opencode`; remove with `npx @toolu/plugins remove toolu --host opencode --yes`.

## What you get

- **Before each tool call.** `tool.execute.before` runs toolu's core gates and the selected plugins' registry modules on `bash`, `read`, `grep`, `glob`, `edit`, `write`, `apply_patch`, `task` and MCP tools. A refusal stops the call before it has any side effect, and a toolu allow never overrides your own `permission` rules.
- **After each tool call.** `tool.execute.after` appends post-edit quality diagnostics and gate advice to the result, and records gate state that later commits and pushes are checked against. It cannot undo an action that already ran.
- **Context and surfaces.** The selected plugins' startup instructions, prompt reminders and compaction context reach the model. Their skills, agents and commands are added through the `config` hook.

Each plugin's status and its host-specific limitations are in [docs/opencode.md § Plugin support](https://github.com/Falconiere/toolu/blob/main/docs/opencode.md#plugin-support). The required real-host acceptance in CI checks that table.

## Requirements

- OpenCode `opencode-ai@1.18.34`, the only verified version
- Bun 1.4.x and git
- macOS or Linux. Windows is not supported.

Per-gate tools (rustfmt, oxlint, ruff, …) are whatever the plugins you enable require.

## Upgrading from 7.7.2 or earlier

Those releases targeted OpenCode V2 and do not load on this line, and this package does not load on V2. Follow the [migration guide](https://github.com/Falconiere/toolu/blob/main/docs/opencode-migration.md); it covers what toolu owns and how to roll back.

## Overriding the plugin root

The bundled tree is used by default. To run against a checkout instead, when working on toolu itself, set `TOOLU_REPO_ROOT` to the clone or pass `repoRoot` as a plugin option. Both take precedence over the bundled copy.

Full documentation: **[docs/opencode.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode.md)**

MIT © Falconiere Barbosa
