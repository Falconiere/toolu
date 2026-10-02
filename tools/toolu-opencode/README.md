# @toolu/opencode

The [toolu](https://github.com/Falconiere/toolu) adapter for [OpenCode](https://opencode.ai) runs toolu's TypeScript dispatcher in `tool.execute.before`, so an edit that violates a gate is denied before bytes change.

The adapter uses the documented plugin API (`opencode-ai@1.18.34`, `@opencode-ai/plugin@1.18.34`) pinned in [docs/opencode-host-contract.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode-host-contract.md). [#336](https://github.com/Falconiere/toolu/issues/336) established the native plugin entrypoint.

## Generated OpenCode surface

`bun run generate:opencode-surface` builds `generated/` from all 16 plugin manifests. The catalog records 18 skills, five subagents, four commands, three plugins with no Markdown surface, and the Claude-only statusline setup command excluded for [OP-25](https://github.com/Falconiere/toolu/issues/359). `bun run check:opencode-surface` detects drift. `bun run probe:opencode-surface` loads the generated Markdown in an isolated pinned OpenCode 1.18.34 profile and checks discovery and parsed agent/command configuration.

Skill names are unique, at most 64 characters, and use lowercase letters, digits, and single hyphens. The generator preserves YAML metadata and agent permissions, maps Claude tool lists to restrictive OpenCode permissions, and lets agents inherit the configured host model instead of emitting Claude model aliases. Local linked resources are copied into `generated/`; known skill references and source paths point to generated IDs and files. The generated catalog is package content. Native installation and enabled-plugin selection are handled by [OP-11](https://github.com/Falconiere/toolu/issues/345).

## Install

```bash
opencode plugin add @toolu/opencode
```

Then choose which toolu plugins are active, in your project:

```jsonc
// .opencode/toolu/plugins.json
{ "version": 1, "enabled": ["toolu"] }
```

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
