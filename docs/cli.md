# `toolu` CLI

**Status:** in progress. The commands below work against Claude Code and Codex
today, and the [README](../README.md#install) install snippets use them.
OpenCode wiring and the interactive prompts are not built yet.

`--host opencode` exits `2`. OpenCode has its own plugin CLI, but it installs npm
packages rather than marketplace entries, so the thirteen bash plugins are not
addressable through it — what it installs is the `@toolu/opencode` bridge. Until
the adapter lands, run `opencode plugin add @toolu/opencode` and write
`.opencode/toolu/plugins.json` yourself; see [docs/opencode.md](opencode.md).

Design: `docs/toolu/specs/2026-09-22-npx-toolu-cli-design.md` (untracked; `docs/toolu/` is gitignored).

## Install

Published to npm as `@toolu/plugins` when a GitHub Release is published:

```bash
npx @toolu/plugins install
```

npx fetches the newest release, so no `@latest` tag is needed. The one
exception is a project that already depends on `@toolu/plugins`: there npx runs
that pinned copy. The package declares a single command, `toolu`, and npx passes
`install` straight to it. A global install (`npm install -g @toolu/plugins`)
gives you `toolu install`.

The package is scoped because npm rejects the unscoped name `toolu` as too
similar to the existing package `toml`. It replaces `@toolu/cli`, which used a
`toolu plugins install` grammar and is no longer published. If you installed
`@toolu/cli` globally, run `npm uninstall -g @toolu/cli` first: both packages
provide the `toolu` command, and npm refuses to overwrite one package's command
with another's.

**Why it publishes from `tools/toolu-cli/npm`.** npx checks the local project
tree before the registry. If the repository root or a declared workspace carried
the name `@toolu/plugins`, npx would treat it as installed inside any toolu
checkout and fail with `sh: toolu: command not found`, which is what the old
`@toolu/cli` did. So the dev workspace at `tools/toolu-cli` is private and named
`toolu-cli`, and the published manifest lives in its `npm/` folder, which no
workspace declares. `tooling/__tests__/npx-invocation.bats` runs npm's own
lookup to prove no local package matches, and fails if a documented command
adds a tag, a version, or the old name. The one folder where npx still fails is
`tools/toolu-cli/npm` itself. npm treats a folder with its own `package.json` as
the project, and that manifest is the published package.

To run unreleased code from a checkout, skip npx:

```bash
bun tools/toolu-cli/src/cli.ts install --dry-run
```

[`@toolu/core`](https://www.npmjs.com/package/@toolu/core) and
[`@toolu/opencode`](https://www.npmjs.com/package/@toolu/opencode) publish from
the same release. `@toolu/conformance` stays private — it is an internal
harness.

The `@toolu/plugins` tarball is five files: `package.json`, `README.md`,
`LICENSE`, `dist/cli.js`, and `assets/marketplace.json`. The bash `plugins/` tree is
deliberately excluded, because Claude Code and Codex fetch plugin content
through their own host CLIs — it ships inside `@toolu/opencode` instead, which
is the one runtime that reads it. `bun run test:pack` fails if anything else
appears in any of the three.

## Grammar

The command comes first. The package name already says what it acts on:

```
npx @toolu/plugins install [name...]   Install plugins, core first (no names = all)
npx @toolu/plugins list                Catalog joined with what the host reports
npx @toolu/plugins remove <name...>    Uninstall, requires --yes
npx @toolu/plugins update [name...]    Update only what is behind the marketplace
```

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, or `opencode`. Detected when omitted. |
| `--scope <scope>` | `user`, `project`, `local`. **Claude Code only** — passing it with another host exits `2`. |
| `--config <path>` | Replay a `.toolu/plugins.json` selection. |
| `--yes`, `-y` | Confirm a destructive or command-declaring operation. |
| `--dry-run` | Print the host commands in order; run none of them. |
| `--no-input` | Never prompt; fail listing what is missing. Use in CI. |
| `--json` | Machine-readable output where supported. |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Everything succeeded or was already satisfied |
| `1` | Something failed; every failure is named on stderr |
| `2` | Usage error: unknown command, flag, host, or plugin |
| `3` | Required input missing, or an ambiguous host with no `--host` |
| `130` | An interactive prompt was cancelled |

## Behavior worth knowing

**Install order comes from the catalog, not the CLI.** `.claude-plugin/marketplace.json`
declares that `python-quality`, `rust-quality`, `ts-quality`, and `pr-babysit`
depend on `toolu`. Requesting a dependent installs its dependency first. Adding a
plugin to the catalog needs no CLI change.

**Already installed is not an error.** A plugin already present is reported and
left alone. When its version differs from the one the marketplace offers, both
versions are named and it is still left alone — `update` is how you move it. Exit stays `0`.

**Only a core failure stops dependents.** If `toolu` itself fails to install, its
four dependents are skipped, because they cannot succeed without it. Any other
failure is recorded and the remaining plugins are still attempted. Either way the
exit code is `1` and every failure is named.

**Host ambiguity is never resolved silently.** With more than one host on `PATH`
and no `--host`, the CLI exits `3` and names the candidates rather than guessing.

**`-y` is never supplied on your behalf.** Claude Code requires `-y` to accept a
marketplace-declared command without confirmation. That flag exists so a person
approves an arbitrary command, so the CLI forwards it only when you pass `--yes`
yourself, never because it noticed there is no terminal.

## `.toolu/plugins.json`

Sits beside the tracked `.toolu/skills/` convention.

```json
{
  "version": 1,
  "generatorVersion": "6.5.0",
  "host": "claude",
  "enabled": ["toolu", "rust-quality", "ts-quality"]
}
```

`version` is the file format, and an unknown value is rejected rather than
guessed at. `generatorVersion` records the CLI that wrote it; a file from a newer
major is rejected with both versions named. `host` is advisory — `--host` wins.
Writes are atomic (temp file, then rename), so a concurrent reader sees either
the old file or the new one, never a partial one.
