# @toolu/opencode

The [toolu](https://github.com/Falconiere/toolu) adapter for [OpenCode](https://opencode.ai) runs toolu's TypeScript dispatcher in `tool.execute.before`, so an edit that violates a gate is denied before bytes change.

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

Restart OpenCode. The package carries plugin manifests, settings and committed Bun bundles, so there is no clone and no `TOOLU_REPO_ROOT` to export.

## What you get

The `toolu` plugin brings protected-file blocking and shell command gates through `permission.evaluate`. Bundled registry plugins such as `ast-grep` also contribute pre-tool decisions. OpenCode post-tool quality checks are not wired through this permission hook.

Enforcement scope matches the fixture evidence in [#212](https://github.com/Falconiere/toolu/issues/212); it is not yet the full Claude Code and Codex hook surface.

## Requirements

- OpenCode `v2.0.12`
- Bun 1.4.x and git
- macOS or Linux. Windows is not supported.

Per-gate tools (rustfmt, oxlint, ruff, …) are whatever the plugins you enable require.

## Overriding the plugin root

The bundled tree is used by default. To run against a checkout instead — when working on toolu itself — set `TOOLU_REPO_ROOT` to the clone, or pass `repoRoot` as a plugin option. Both take precedence over the bundled copy.

Full documentation: **[docs/opencode.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode.md)**

MIT © Falconiere Barbosa
