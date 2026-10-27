# The plugin installer (`toolu plugins`)

This guide covers `toolu plugins`, the namespace of the native `toolu` binary
that installs toolu's plugins on each host (#438). The generated reference of
every verb and flag is [plugins.md](plugins.md). The
[README](../../README.md#install) install snippets use these commands.

Claude Code, Codex and Muse are driven through their own plugin CLIs; Cursor has
none, so toolu copies plugins into its local plugins directory. The code is
`crates/plugins` (`toolu-plugins`); Muse is [below](#muse) and Cursor
[after it](#cursor).

Design: `docs/toolu/specs/2026-10-12-toolu-plugins-in-rust-design.md`.

## Install

Install the native binary once ([docs/install.md](../install.md)), then the plugins:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
brew install falconiere/tap/toolu
toolu plugins install
```

The npm package `@toolu/plugins` is deprecated. Its command prints those lines
and exits `69`. It does not install plugins and it ships no binary. Remove a
global install, including one from before #438 that put `toolu` on `PATH`,
with `npm uninstall -g @toolu/plugins`. That leftover fails the native
`--hook-protocol` check, so SessionStart and `toolu doctor` skip it
([Installing toolu](../install.md#agent-command-shell)).

`@toolu/core`, the old TypeScript library, is deprecated and no longer published.

To run unreleased code from a checkout, build the binary and call it:

```bash
cargo build -p toolu-cli
./target/debug/toolu plugins install --dry-run
```

## Grammar

The verb comes first:

```
toolu plugins install [name...]   Install plugins, core first (terminal: pick; else all)
toolu plugins list                Catalog joined with what the host reports
toolu plugins remove <name...>    Uninstall, requires --yes
toolu plugins update [name...]    Update toolu's plugins on every host that has them
```

`toolu plugins install` is the install verb, and the same shape covers `list`, `remove`, and `update`.

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, `muse` or `cursor`. Detected when omitted; see [Choosing hosts](#choosing-hosts). Without `--host`, `update` acts on every host that has toolu plugins. `hermes` is not an installer target and exits `64`. |
| `--scope <scope>` | `user`, `project` or `local` (Claude Code only). Any scope with Codex, Muse or Cursor exits `64`, except `update` without `--host`, which does not use it. |
| `--approve` | `install` and `update` only: approve each plugin's pending hooks on Muse. Does nothing on Claude Code and Codex. |
| `--config <path>` | `install` only: replay a [`.toolu/plugins.json`](#tooluplugins-json) selection. Not combined with names. |
| `--catalog <path>` | Read the plugin catalog from a file instead of the one built into the binary. |
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
| `64` | Usage error or missing input: unknown command, flag, host, or plugin, an ambiguous host for `install`, `list` or `remove` with no terminal or with `--no-input`, a removal without `--yes`, a bad `--config` file |
| `69` | The deprecated `@toolu/plugins` command. It installs nothing. |

The TypeScript installer used `2`, `3` and `130` for these. Scripts that test for
those codes must test for `64` and `1`.

## JSON

`--json` prints one document. `list` prints an array of
`{ "name", "installed", "version"?, "enabled" }`. `install` prints
`{ "dryRun", "hosts": [{ "host", "steps": [...] }] }`. `remove` prints
`{ "host", "steps": [...] }`, and so does `update --host <id>`. `update` without
`--host` prints the `install` document, one entry per host it updated. A step is
`{ "name", "outcome", "detail", "argv", "plan"? }`, and a removal step has
`removed` (a boolean) instead of `outcome`. A run that stops on an error prints
the CLI's `{ "error": { "code", "name", "message" } }` document.

**Compatibility.** Before 8.6, `update --json` without `--host` printed
`{ "host", "steps" }` when one host was installed and refused when there were
several. It now always prints the `install` document. A script that reads one
host's steps should pass `--host`.

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

**Host ambiguity is never resolved silently.** `install`, `list` and `remove`
ask when several hosts are found and there is no `--host`
([Choosing hosts](#choosing-hosts)). Without a terminal, or with `--no-input`
or `--json`, they exit `64` and name the candidates rather than guessing.
`update` does not ask: it updates every host that has toolu plugins
([Updating every host](#updating-every-host)).

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
leads to the remove; if Codex cannot list its marketplaces (this needs Codex
0.164.0 or later), or the pinned add fails for another reason, the run fails
naming the tag and changes nothing. With no `toolu` marketplace and a failed
add, `install` still tries each plugin and names the add on a plugin that
fails, while `update` fails. If
the pinned add fails after the remove, the unpinned marketplace is added back
and the run fails naming the tag. A `toolu`
marketplace from any other source, such as a local checkout, is left as it is.
`toolu --host codex doctor` warns `plugins` while the marketplace is not
pinned to this binary's tag.

**Dry runs run nothing.** `--dry-run` prints the commands a real run would
run, for `install`, `remove` and `update` alike (an `update` leaves out plugins
already at the offered version; Claude Code offers none, so its dry run lists
every installed toolu plugin). The TypeScript installer ignored the flag on
`remove` and `update` for Claude Code and Codex. Muse had no TypeScript installer.

**Each host command has 600 seconds.** A command that runs longer is a failed
step whose detail says `timed out after 600 s`. Ctrl-C stops the running host
command and `toolu` with it.

**Progress is live at a terminal.** See [Terminal output](#terminal-output).

## Choosing hosts

`--host` always wins. Without it, one host on `PATH` (`claude`, `codex`,
`muse`) is used without asking. When several are found, a terminal asks:

- `install` offers every supported host on the machine: Claude Code, Codex and
  Muse on `PATH`, and Cursor when its config directory (`~/.cursor`) exists or
  `cursor-agent` is on `PATH`. Several can be chosen.
- `list` and `remove` ask for one host. They offer Cursor only when toolu
  installed a plugin there, so a leftover `~/.cursor` never adds the question.
- An `install` with no plugin names and no `--config` then asks which plugins,
  every catalog entry selected.

At a terminal the question is a checkbox list. ↑/↓ (or `k`/`j`) move, space
toggles, `a` toggles all, Enter confirms, and `q`, Esc or Ctrl-C cancels (exit
`1`, `cancelled`). Every option starts selected, and confirming with nothing
selected asks again. The answered list collapses to one line. When the terminal
cannot be put in raw mode, or stdout is not a terminal, the question is numbered
lines instead: Enter takes the default, `1 3` or `1,3` picks, `q` cancels.

Without a terminal only `PATH` hosts count, so scripts resolve as they always
did. A script names Cursor with `--host cursor`.

## Updating every host

`toolu plugins update` never asks which host. Without `--host` it updates every
detected host that has toolu plugins, in a section each. That is the `PATH`
hosts, and Cursor when toolu installed a plugin there.

- Each host's toolu plugins are read before anything is prepared, pinned or
  cloned. A host without them is only noted (`muse: no toolu plugins
  installed; skipped`). If no host has any, the run exits `1`.
- Only toolu's own plugins are updated: Claude Code entries from other
  marketplaces are never touched, and a plugin installed in several scopes is
  updated once. A plugin the catalog retired is named with the command that
  removes it on that host instead of being updated.
- Names update only where they are installed. A name installed on no host
  exits `1`.
- A host that fails (for example, its marketplace cannot be pinned) gets a failed
  step named after the host. The other hosts still run, and the exit is `1`.
- `--scope` is accepted for Claude Code and not used by `update`: Claude Code
  picks the scope of each update itself.

On Claude Code, each update runs `claude plugin update <name>@toolu --json`, and
its result line decides `current at <version>`, `updated <old> → <new>`, or
skipped with Claude's reason. This needs a Claude Code whose `plugin update` takes
`--json` (verified with 2.1.311); an older one fails each update with its own
message.

## Terminal output

When stdout is a terminal, `toolu plugins` and `toolu status` draw on rails. A
`┌  toolu plugins <verb>` intro opens, each host is a `◇` section, and each plugin
gets a glyph: `✔` done, `●` unchanged, `▲` left alone, `✘` failed, `○` would
run. A `└` line closes with the counts and the time taken. While host commands
run, each host shows a bar that advances per plugin and a spinner line naming
the plugin at work. The bar draws on the terminal (`/dev/tty`), never stdout, and
clears before the report prints. The cursor is never hidden.

`NO_COLOR` (non-empty) drops color but keeps the rails. `TERM=dumb`, `--json`
and piped or redirected stdout print the plain report, unchanged.
`--quiet` drops the progress.

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
