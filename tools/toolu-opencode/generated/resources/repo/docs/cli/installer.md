# The plugin installer (`npx @toolu/plugins`)

This guide covers the Node installer published to npm as `@toolu/plugins`, which
installs toolu's plugins on each host. The commands of the Rust `toolu` binary
are generated in [README.md](README.md); `toolu plugins` replaces this installer
in #438.

The commands below work against Claude Code, Codex and OpenCode, and the
[README](../../README.md#install) install snippets use them. Interactive host and
plugin prompts are live on a TTY. Claude Code and Codex are driven through their
own plugin CLIs; OpenCode has none for this, so the CLI edits its documented
config files ([OpenCode](#opencode)).

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
workspace declares. `tooling/src/__tests__/npx-invocation.test.ts` runs npm's own
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
`LICENSE`, `dist/cli.js`, and `assets/marketplace.json`. The `plugins/` tree is
deliberately excluded, because Claude Code and Codex fetch plugin content
through their own host CLIs — it ships inside `@toolu/opencode` instead, which
is the one runtime that reads it. `bun run test:pack` fails if anything else
appears in any of the three.

## Grammar

The command comes first. The package name already says what it acts on:

```
npx @toolu/plugins install [name...]   Install plugins, core first (TTY: pick; else all)
npx @toolu/plugins list                Catalog joined with what the host reports
npx @toolu/plugins remove <name...>    Uninstall, requires --yes
npx @toolu/plugins update [name...]    Update only what is behind the marketplace
```

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, or `opencode`. Detected when omitted. |
| `--scope <scope>` | Claude Code: `user`, `project`, `local`. OpenCode: `user` (global config, the default) or `project`. Any other combination, and any scope with Codex, exits `2`. |
| `--config <path>` | Replay a `.toolu/plugins.json` selection. |
| `--yes`, `-y` | Confirm a destructive or command-declaring operation. |
| `--dry-run` | Print the host commands in order (OpenCode: the planned config edits); change nothing. |
| `--no-input` | Never prompt; fail listing what is missing. Use in CI. |
| `--json` | Machine-readable output where supported. |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Everything succeeded or was already satisfied |
| `1` | Something failed; every failure is named on stderr |
| `2` | Usage error: unknown command, flag, host, or plugin |
| `3` | Required input missing, or an ambiguous host with no TTY / `--no-input` |
| `130` | An interactive prompt was cancelled |

## Behavior worth knowing

**Live install progress.** Terminals show an animated bar for each host, with
the current plugin and completed/total count (including dependencies). Already
installed, failed, and skipped plugins count as processed; the final report
shows each outcome. Redirected output, `TERM=dumb`, `--json`, and dry runs omit
the animation. `--no-input` disables prompts but still shows progress on a terminal.

**Install order comes from the catalog, not the CLI.** `.claude-plugin/marketplace.json`
declares that `python-quality`, `rust-quality`, `ts-quality`, `pr-babysit`,
`delivery-flow`, and `epic-orchestrator` depend on `toolu`. `delivery-flow`
also depends on `toolu-review`, `pr-babysit`, and `brainstorm`; `epic-orchestrator` depends on
`delivery-flow` and `pr-babysit`. Requesting a dependent installs its dependency first. Adding a
plugin to the catalog needs no CLI change.

**Already installed is not an error.** A plugin already present is reported and
left alone. When its version differs from the one the marketplace offers, both
versions are named and it is still left alone — `update` is how you move it. Exit stays `0`.

**Only a core failure stops dependents.** If `toolu` itself fails to install, its
dependents are skipped, because they cannot succeed without it. Any other
failure is recorded and the remaining plugins are still attempted. Either way the
exit code is `1` and every failure is named.

**Host ambiguity is never resolved silently.** With more than one wired host on
`PATH` and no `--host`, a TTY prompts: `install` multi-selects hosts (and, when
no plugin names were given, multi-selects plugins with every catalog entry
selected by default), then runs once per chosen host with a sectioned report.
`list`, `remove`, and `update` single-select one host. Without a TTY, or with
`--no-input`, the CLI exits `3` and names the candidates rather than guessing.
Cancelling a prompt exits `130`.

**`-y` is never supplied on your behalf.** Claude Code requires `-y` to accept a
marketplace-declared command without confirmation. That flag exists so a person
approves an arbitrary command, so the CLI forwards it only when you pass `--yes`
yourself, never because it noticed there is no terminal.

## OpenCode

The supported host (`opencode-ai@1.18.34`, see the
[host contract](../opencode-host-contract.md)) has no `plugin add`, `list`,
`update` or `remove` command. The CLI therefore edits the config files OpenCode
documents and never runs the host. One npm package, `@toolu/opencode`, carries
every plugin. The CLI manages two things: that package's entry in a `plugin`
array, and a plugin selection file `toolu/plugins.json`
(`{ "version": 1, "enabled": [...] }`). Restart OpenCode to load a change. A `@toolu/opencode` entry that OpenCode 2.x's own
plugin command wrote is an ordinary entry here: `update` rewrites it
([migration guide](../opencode-migration.md)).

| Verb | Effect |
|------|--------|
| `install` | Adds `@toolu/opencode@<CLI version>` to the `plugin` array once. Without a selection file every plugin is enabled. |
| `install <name...>` | Adds the package if needed and enables the names plus their dependencies. A fresh install selects only those; otherwise they join the current selection. |
| `list` | The effective package entry and selection, and per plugin whether it is enabled. |
| `update` | Rewrites every toolu entry in scope to the CLI's version. A `[spec, options]` tuple keeps its options. Exits `1` when the package is not configured. |
| `remove <name...>` | Disables the names in the selection. Refused with exit `1` while an enabled plugin still depends on one. |
| `remove toolu` | Removes the package entry and keeps the selection file, so a reinstall restores it. |

**Where it writes.** `--scope user`, the default, edits the global directory
`${XDG_CONFIG_HOME:-~/.config}/opencode/`. It picks the highest-priority file
that defines `plugin` (`opencode.jsonc` over `opencode.json` over
`config.json`), else the last of those that exists, else a new `opencode.json`.
The selection is `toolu/plugins.json` under the same root that
`@toolu/opencode` reads (see [Roots](../opencode.md#roots-and-helper-environment)).
`--scope project` writes at the git worktree root, even from a subdirectory. It
picks the last of `opencode.json`, `opencode.jsonc`, `.opencode/opencode.json`
and `.opencode/opencode.jsonc` that defines `plugin`. The selection is
`.opencode/toolu/plugins.json`. `--scope local` exits `2`. Without `--scope`,
`install <name...>` and `remove <name...>` edit the project selection when one
exists, since that is the one `@toolu/opencode` reads there. With `--scope user`
in such a project, the step says the global change has no effect here.

**How OpenCode merges, so the CLI does too.** In the global directory only the
highest-priority file that defines `plugin` counts. Global and project arrays
then concatenate, and for the same package the last entry wins. An empty
`"plugin": []` clears every earlier entry, so `remove` deletes a `plugin` key it
would leave empty. `list` reports the entry OpenCode would actually load, and
`install` says so when a project file shadows the global one, or when filling a
project's empty `plugin` array lets earlier plugins load again.

**What survives.** Edits go through a JSONC parser that changes only the
`plugin` array or the selection. Comments, formatting, other keys and other
plugin entries stay as they were. Writes are atomic and follow a symlinked
config to its target. A file that is not valid JSONC, a `plugin` key that is not
an array, or an invalid selection file exits `1`, names the file, and writes
nothing.

**Rules worth knowing.**

- Running `install` again reports `already configured` and changes nothing.
- An entry pinned to another version is reported with both specs and left
  alone. `update` is how you move it.
- When the package is configured in both scopes, `remove` and `update` need
  `--scope`. Without it they exit `2` and write nothing.
- Removing something that is not there succeeds and changes nothing.
- `skills.<name>: false` in `.opencode/toolu.config.json` still turns a plugin
  off, the same way it does for the adapter.
- `TOOLU_OPENCODE_PACKAGE` overrides the spec written, for testing a packed
  tarball. It must name `@toolu/opencode`, or the CLI exits `2`.

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
