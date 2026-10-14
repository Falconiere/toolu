# The plugin installer (`toolu plugins`, `npx @toolu/plugins`)

This guide covers `toolu plugins`, the namespace of the native `toolu` binary
that installs toolu's plugins on each host (#438), and `@toolu/plugins`, the npm
package that runs it. The generated reference of every verb and flag is
[plugins.md](plugins.md). The [README](../../README.md#install) install snippets
use these commands.

Claude Code and Codex are the supported hosts, driven through their own plugin
CLIs. The code is `crates/plugins` (`toolu-plugins`).

Design: `docs/toolu/specs/2026-10-12-toolu-plugins-in-rust-design.md`.

## Install

Install the native binary once ([docs/install.md](../install.md)), then the plugins:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
toolu plugins install
```

The same verbs run through npm, which is handy where `npx` is the habit:

```bash
npx @toolu/plugins install
```

`@toolu/plugins` is a thin Node wrapper. It finds the native `toolu` and runs
`toolu plugins <args>`, passing the plugin catalog it ships
(`assets/marketplace.json`) with `--catalog`. It looks for `toolu` the way the plugins'
hook launcher does. A non-empty `TOOLU_BIN` is the only candidate. Otherwise it
tries `/opt/homebrew/bin`, `/usr/local/bin`, `/home/linuxbrew/.linuxbrew/bin`,
`~/.local/bin`, then every `toolu` on `PATH`, in that order.
A candidate counts only if it is not the wrapper itself, `toolu --hook-protocol`
prints an integer, and `toolu plugins install --help` succeeds. With no such
binary the wrapper exits `69` and names the install commands. It never
downloads a binary. It exits with the native run's status, forwards Ctrl-C,
`SIGTERM` and `SIGHUP` to it, and exits 128 plus the signal's number when a
signal ends it. `npx @toolu/plugins --version` prints the wrapper's own
version.

npx fetches the newest release, so no `@latest` tag is needed. The one
exception is a project that already depends on `@toolu/plugins`: there npx runs
that pinned copy. The package declares a single command, `toolu-plugins`, and
npx passes `install` straight to it. A global install
(`npm install -g @toolu/plugins`) gives you `toolu-plugins install`.

The command used to be called `toolu`, the same name as the native binary, so a
global npm install could shadow the binary in an agent command shell. A script
that ran the old global command's verbs (`install`, `list`, `remove`, `update`)
must now run them as `toolu plugins <verb>` on the native binary, or as
`toolu-plugins <verb>` from the npm package. If you
installed an earlier release globally, run `npm uninstall -g @toolu/plugins`,
then install it again. A leftover `toolu` from that package fails the native
`--hook-protocol` check, so SessionStart, `toolu doctor` and the wrapper all
skip it ([Installing toolu](../install.md#agent-command-shell)).

The package is scoped because npm rejects the unscoped name `toolu` as too
similar to the existing package `toml`. It replaces `@toolu/cli`, which used a
`plugins install` noun grammar and is no longer published.

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

To run unreleased code from a checkout, build the binary and call it:

```bash
cargo build -p toolu-cli
./target/debug/toolu plugins install --dry-run
```

[`@toolu/core`](https://www.npmjs.com/package/@toolu/core) publishes from the
same release. `@toolu/conformance` stays private, because it is an internal
harness.

The `@toolu/plugins` tarball is five files: `package.json`, `README.md`,
`LICENSE`, `dist/cli.js`, and `assets/marketplace.json`. The `plugins/` tree is
deliberately excluded, because Claude Code and Codex fetch plugin content
through their own host CLIs. The gate's `pack` step fails if anything else
appears in either package.

## Grammar

The verb comes first; with npx the package name already says what it acts on:

```
toolu plugins install [name...]   Install plugins, core first (terminal: pick; else all)
toolu plugins list                Catalog joined with what the host reports
toolu plugins remove <name...>    Uninstall, requires --yes
toolu plugins update [name...]    Update only what is behind the marketplace
```

`npx @toolu/plugins install` is `toolu plugins install`, and so on for every verb.

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude` or `codex`. Detected when omitted. `cursor` and `hermes` are not installer targets and exit `64`. |
| `--scope <scope>` | `user`, `project` or `local` (Claude Code only). Any scope with Codex exits `64`. |
| `--config <path>` | `install` only: replay a [`.toolu/plugins.json`](#tooluplugins-json) selection. Not combined with names. |
| `--catalog <path>` | Read the plugin catalog from a file instead of the one built into the binary. The npm wrapper passes its own. |
| `--yes`, `-y` | Confirm a removal. |
| `--dry-run` | Print the host commands in order; change nothing. |
| `--no-input` | Never prompt; fail naming what is missing. Use in CI. |
| `--json` | One JSON document on stdout. |

## Exit codes

The exit codes are those of the `toolu` CLI contract ([README.md](README.md)):

| Code | Meaning |
|------|---------|
| `0` | Everything succeeded or was already satisfied |
| `1` | Something failed (every failure is named), or a prompt was cancelled |
| `64` | Usage error or missing input: unknown command, flag, host, or plugin, an ambiguous host with no terminal or with `--no-input`, a removal without `--yes`, a bad `--config` file |
| `69` | The wrapper found no native `toolu` |

The TypeScript installer used `2`, `3` and `130` for these. Scripts that test for
those codes must test for `64` and `1`.

## JSON

`--json` prints one document. `list` prints an array of
`{ "name", "installed", "version"?, "enabled" }`. `install` prints
`{ "dryRun", "hosts": [{ "host", "steps": [...] }] }`. `remove` and `update`
print `{ "host", "steps": [...] }`. A step is `{ "name", "outcome", "detail",
"argv", "plan"? }`, and a removal step has `removed` (a boolean) instead of
`outcome`. A run that stops on an error prints the CLI's
`{ "error": { "code", "name", "message" } }` document.

## Behavior worth knowing

**Install order comes from the catalog, not the CLI.** `.claude-plugin/marketplace.json`
declares that `python-quality`, `rust-quality`, `ts-quality`, `pr-babysit`,
`delivery-flow`, and `epic-orchestrator` depend on `toolu`. `delivery-flow`
also depends on `toolu-review`, `pr-babysit`, and `brainstorm`; `epic-orchestrator` depends on
`delivery-flow` and `pr-babysit`. Requesting a dependent installs its dependency first. Adding a
plugin to the catalog needs no CLI change.

**Already installed is not an error.** A plugin already present is reported and
left alone. When its version differs from the one the marketplace offers, both
versions are named and it is still left alone. `update` is how you move it.
Exit stays `0`.

**Only a core failure stops dependents.** If `toolu` itself fails to install, its
dependents are skipped, because they cannot succeed without it. Any other
failure is recorded and the remaining plugins are still attempted. Either way the
exit code is `1` and every failure is named.

**Host ambiguity is never resolved silently.** With more than one wired host on
`PATH` and no `--host`, a terminal prompts. `install` asks which hosts and then
runs once per chosen host with a sectioned report. Separately, an `install` on a
terminal with no plugin names and no `--config` asks which plugins, every
catalog entry selected by default, whatever the host.
`list`, `remove`, and `update` ask for one host. The prompts are numbered lines
on the terminal (`/dev/tty`): Enter takes the default, `1 3` or `1,3` picks,
and `q` cancels. Without a terminal, or with `--no-input` or `--json`, the run
exits `64` and names the candidates rather than guessing.

**Dry runs run nothing.** `--dry-run` prints the commands a real run would
run, for `install`, `remove` and `update` alike (an `update` leaves out plugins
already at the offered version). The TypeScript installer ignored the flag on
`remove` and `update` for Claude Code and Codex.

**Each host command has 600 seconds.** A command that runs longer is a failed
step whose detail says `timed out after 600 s`. Ctrl-C stops the running host
command and `toolu` with it.

**No live progress bar.** The TypeScript installer animated a progress line. The
native report prints once, when the run ends.

## `.toolu/plugins.json`

`toolu plugins install --config .toolu/plugins.json` replays a selection that
sits beside the tracked `.toolu/skills/` convention:

```json
{
  "version": 1,
  "generatorVersion": "6.5.0",
  "host": "claude",
  "enabled": ["toolu", "rust-quality", "ts-quality"]
}
```

The run installs `enabled` with its dependencies, as if the names had been
given, and an empty list installs nothing. `version` is the file format: an
unknown or missing value exits `64` rather than being guessed at.
`generatorVersion` records the CLI that wrote the file, and a file from a newer
major exits `64` with both versions named. `host` is used when `--host` is
absent; `--host` wins. A file that is missing, is not JSON or is malformed also
exits `64`. Nothing writes this file. The TypeScript installer parsed
`--config` but never read it.
