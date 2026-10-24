# The plugin installer (`toolu plugins`, `npx @toolu/plugins`)

This guide covers `toolu plugins`, the namespace of the native `toolu` binary
that installs toolu's plugins on each host (#438), and `@toolu/plugins`, the npm
package that runs it. The generated reference of every verb and flag is
[plugins.md](plugins.md). The [README](../../README.md#install) install snippets
use these commands.

Claude Code, Codex and Muse are driven through their own plugin CLIs; Cursor has
none, so toolu copies plugins into its local plugins directory. The code is
`crates/plugins` (`toolu-plugins`); Muse is [below](#muse) and Cursor
[after it](#cursor).

Design: `docs/toolu/specs/2026-10-12-toolu-plugins-in-rust-design.md`.

## Install

Install the native binary once ([docs/install.md](../install.md)), then the plugins:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
toolu plugins install
```

The same verbs run through npm, which is handy where `npx` is the habit.
No separate binary install is needed: the package ships a native `toolu` for
each supported platform:

```bash
npx @toolu/plugins install
```

`@toolu/plugins` is a thin Node wrapper. It runs `toolu plugins <args>`,
passing the plugin catalog it ships (`assets/marketplace.json`) with
`--catalog`. It prefers the binary the package ships
(`bin/<os>-<arch>/toolu`), then looks for an installed `toolu` the way the
plugins' hook launcher does. A non-empty `TOOLU_BIN` is the only candidate and
always wins. Otherwise the bundled binary runs when it answers the probes;
then `/opt/homebrew/bin`, `/usr/local/bin`,
`/home/linuxbrew/.linuxbrew/bin`, `~/.local/bin`, then every `toolu` on
`PATH`, in that order.
A candidate counts only if it is not the wrapper itself, `toolu --hook-protocol`
prints an integer, and `toolu plugins install --help` succeeds. A bundled
binary that fails those probes is skipped for the installed search. To run an
installed `toolu` instead of a working bundled one, set `TOOLU_BIN` to its
path. With no working binary anywhere the wrapper exits `69` and names the
install commands.
It never downloads a binary: everything it runs rides the tarball. It exits
with the native run's status, forwards Ctrl-C, `SIGTERM` and `SIGHUP` to it,
and exits 128 plus the signal's number when a signal ends it.
`npx @toolu/plugins --version` prints the wrapper's own version. A pinned copy
(an older global install, or a project dependency) runs its pinned bundled
binary rather than a newer installed one. There is no Windows binary, so on
Windows the wrapper always exits `69`.

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
`@toolu/cli` did. So the root package is the private `toolu-workspace`, and the
published manifest lives in `tools/toolu-cli/npm`, which no workspace declares.
The wrapper is one committed file, `cli.js`, with no build step.
`crates/cli/tests/npm_wrapper.rs` runs it with Node. The one folder where npx
still fails is `tools/toolu-cli/npm` itself. npm treats a folder with its own
`package.json` as the project, and that manifest is the published package.

To run unreleased code from a checkout, build the binary and call it:

```bash
cargo build -p toolu-cli
./target/debug/toolu plugins install --dry-run
```

`@toolu/core`, the old TypeScript library, is deprecated and no longer published.

The `@toolu/plugins` tarball is nine files: `package.json`, `README.md`,
`LICENSE`, `cli.js`, `assets/marketplace.json`, and the four
`bin/<os>-<arch>/toolu` binaries. The `plugins/` tree is
deliberately excluded, because Claude Code, Codex and Muse fetch plugin content
through their own host CLIs. The gate's `pack` step fails if anything else
appears in it.

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
| `--host <id>` | `claude`, `codex`, `muse` or `cursor`. Detected when omitted (probed in the order claude, codex, muse; Cursor is never probed, so name it with `--host cursor`). `hermes` is not an installer target and exits `64`. |
| `--scope <scope>` | `user`, `project` or `local` (Claude Code only). Any scope with Codex, Muse or Cursor exits `64`. |
| `--approve` | `install` and `update` only: approve each plugin's pending hooks on Muse. Does nothing on Claude Code and Codex. |
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

**`remove` also takes a plugin the marketplace retired** (#615), such as a
leftover 7.11.0 `context7` from before the Rust rewrite. On Claude Code, Codex
and Cursor, a name outside the catalog that is shaped like a plugin name
(lowercase letters and digits, single inner hyphens) goes to the host's own
removal, which only reaches toolu's plugins: `claude plugin uninstall
<name>@toolu`, `codex plugin remove <name>@toolu`, or a Cursor directory with
toolu's install marker. A name the host does not have is a failed step with the
host's message, exit `1`. Muse removes by bare name from any source, so there
`remove` takes catalog names only, and `toolu doctor` names `muse plugins remove
<name>` instead. Any other unknown name exits `64`.

**Codex follows a release tag, not `main`** (#626). `install` and `update`
register Codex's marketplace as `codex plugin marketplace add Falconiere/toolu
--ref v<version>`, the tag of the running `toolu`. At every session start Codex
re-clones a git marketplace whose ref moved and reinstalls its plugins, and a
reinstalled plugin's hooks need review again. A marketplace that followed
`main` therefore asked for a review after every merge, which stopped Codex
epic workers before their first turn. A tag moves only when you upgrade toolu,
so one trust lasts until then.

An existing `toolu` marketplace from `https://github.com/Falconiere/toolu.git`
that follows another ref is removed and added again pinned. Codex keeps the
installed plugins and their hook trust across that, and toolu then adds each
installed toolu plugin again (`re-pinned to v<version>`) so its cache holds the
tag's content. Only Codex's "already added from a different source" refusal
leads to the remove; if Codex cannot list its marketplaces, or the pinned add
fails for another reason, the run fails naming the tag and changes nothing. If
the pinned add fails after the remove, the unpinned marketplace is added back
and the run fails naming the tag. A `toolu`
marketplace from any other source, such as a local checkout, is left as it is.
`toolu --host codex doctor` warns `plugins` while the marketplace is not
pinned to this binary's tag.

**Dry runs run nothing.** `--dry-run` prints the commands a real run would
run, for `install`, `remove` and `update` alike (an `update` leaves out plugins
already at the offered version). The TypeScript installer ignored the flag on
`remove` and `update` for Claude Code and Codex. Muse had no TypeScript installer.

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
major exits `64` with both versions named. `host` (`claude`, `codex`, `muse` or `cursor`)
is used when `--host` is absent; `--host` wins. A file that is missing, is not JSON or is malformed also
exits `64`. Nothing writes this file. The TypeScript installer parsed
`--config` but never read it.

## Muse

`toolu plugins … --host muse` drives `muse plugins …` (#549). Muse has no
GitHub marketplace source that updates in place, so toolu keeps its own
checkout of the release that matches the binary and registers it with Muse
as the marketplace `toolu` (the #541 decision,
[muse-plugin-distribution.md](../research/muse-plugin-distribution.md)):

- **The checkout** is `$XDG_DATA_HOME/toolu/muse-marketplace` (a relative
  `XDG_DATA_HOME` is ignored), else `~/.local/share/toolu/muse-marketplace`: a
  shallow clone of the tag `v<toolu version>` from
  `https://github.com/Falconiere/toolu.git`. `install` and `update` clone it
  when it is absent, or fetch the tag and check it out when it is there, then
  add it as the marketplace `toolu` or refresh that marketplace's snapshot.
  Plugins install as `muse plugins install <name>@toolu`; `update` runs
  `muse plugins update <name>`, and `remove` runs `muse plugins remove <name>`.
  Upgrading the binary and running `toolu plugins update --host muse` moves
  every installed toolu plugin to the new release.
- **Approval.** Muse keeps a plugin's hooks and agent definitions inert until
  `muse plugins approve <name>` trusts them, and asks again when a definition
  changes. After `install` and `update`, toolu reads `muse plugins inspect
  <name>`. With `--approve` it approves any plugin whose capabilities are
  `review_needed` or `modified`, and the step says `approved`. Without it the
  run still exits `0` and the step ends with `awaiting approval: muse plugins
  approve <name>`. A plugin that is already trusted is left alone, so running
  `install --approve` again over an approved plugin asks for nothing.
- **Plugins from elsewhere.** A toolu plugin Muse installed from another source
  is reported as `skew` and left untouched, with the `muse plugins remove`
  and `toolu plugins install` commands that hand it to toolu; `update` without
  names skips it.
- **Failures before any plugin command** exit `1` and leave Muse unchanged:
  neither `XDG_DATA_HOME` nor `HOME` set; a clone or fetch that fails (offline,
  or a development build whose tag does not exist); a checkout directory that
  is not a toolu checkout or was cloned from another origin (toolu never
  deletes it, move it aside); and a `toolu` marketplace Muse already has from
  another source (remove it with `muse plugins marketplace remove toolu`).
- **Releases before Muse manifests.** Muse installs only plugins whose
  directory carries `.muse-plugin/plugin.json`. A release without them gives a
  failed step per plugin with Muse's `not available in marketplace` line.
- `--dry-run` runs no `git` and changes no marketplace; it prints the plugin
  commands, as for the other hosts.

## Cursor

`toolu plugins … --host cursor` (#589). `cursor-agent plugin` manages only
marketplaces, and `/plugin` records installs on the account, so there is no
command to drive. Cursor does load any plugin directory under
`~/.cursor/plugins/local`, and runs the Claude-format `hooks/hooks.json` inside
it. So the verbs work on the filesystem, and need neither `cursor-agent` nor
its keychain login:

- **The checkout** is `$XDG_DATA_HOME/toolu/cursor-marketplace` (else
  `~/.local/share/toolu/cursor-marketplace`): a shallow clone of the release tag
  that matches the binary, as for Muse but in its own directory.
- **`install`** copies `plugins/<name>/` from it into
  `<cursor config>/plugins/local/<name>`, dependencies first, and writes
  `.toolu-install.json` there (tag and version). `list` reads those markers,
  `update` moves a plugin to the binary's tag, `remove --yes` deletes it.
  The cursor config is `~/.cursor`, or `TOOLU_CONFIG_DIR` when set.
- **Plugins that are not toolu's.** A directory without the marker is reported
  as `skew` and never touched.
- **Failures before any copy** (no `git`, a missing tag, a checkout from
  another origin) exit `1` and leave Cursor unchanged. `--dry-run` clones and
  copies nothing.
- A plugin installed through Cursor's own marketplace is cached at
  `~/.cursor/plugins/cache/toolu/<plugin>/<rev>`; `toolu doctor --host cursor`
  lists both kinds, and Cursor also runs plugins installed for Claude Code under
  `~/.claude`, so install in one place only.
