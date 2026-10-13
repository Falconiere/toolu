# @toolu/plugins

Install [toolu](https://github.com/Falconiere/toolu) plugins into Claude Code, Codex and OpenCode.

These need the native `toolu` binary (below); without it the wrapper exits `69`.

```bash
npx @toolu/plugins install              # every catalog plugin, core first
npx @toolu/plugins install rust-quality # one plugin and its dependency
npx @toolu/plugins list                 # catalog joined with what is installed
npx @toolu/plugins remove ast-grep --yes
npx @toolu/plugins update
```

This package is a thin wrapper. It runs `toolu plugins …` on the native `toolu`
binary, which you install once:

```bash
curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
# or
brew install falconiere/tap/toolu
```

Without it, the wrapper names those commands and exits `69`. It never downloads
a binary. With `toolu` installed, `toolu plugins install` does the same as
`npx @toolu/plugins install`. The wrapper passes along the plugin catalog it
ships (`assets/marketplace.json`).

The package's command is `toolu-plugins`, so a global install
(`npm install -g @toolu/plugins`) never shadows the native `toolu` on your
`PATH`. Earlier releases named it `toolu`. If you installed one of them
globally, run `npm uninstall -g @toolu/plugins` and then install again. The
wrapper skips any `toolu` that is itself, or that is not the native CLI.

## What it does

It drives each host's own plugin CLI (`claude plugin install`,
`codex plugin add`) and never writes into their directories itself. Install
order comes from the catalog's dependency edges, so `rust-quality` pulls in
`toolu` first. When the core plugin `toolu` fails to install, the plugins that
depend on it are skipped. Any other failure is reported, and the rest still
install.

OpenCode has no plugin management commands, so `--host opencode` edits its
documented config instead. It adds `@toolu/opencode` to the `plugin` array of
the global config (or the project's, with `--scope project`), and it records
named plugins in a `toolu/plugins.json` selection. Comments, other keys and
other plugins in those files are kept.

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, or `opencode`. Detected when omitted. |
| `--scope <scope>` | Claude Code: `user`, `project`, `local`. OpenCode: `user` (default) or `project`. Not for Codex. |
| `--config <path>` | `install` only: replay a `.toolu/plugins.json` selection. |
| `--yes`, `-y` | Confirm a removal. |
| `--dry-run` | Print the host commands (OpenCode: the planned config edits); change nothing. |
| `--no-input` | Never prompt; fail naming what is missing. For CI. |
| `--json` | One JSON document on stdout. |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Succeeded, or already satisfied |
| `1` | Something failed (every failure is named), or a prompt was cancelled |
| `64` | Usage error or missing input: unknown command, flag, host or plugin, an ambiguous host with no terminal or with `--no-input`, a removal without `--yes` |
| `69` | The native `toolu` binary is missing (wrapper only) |

Full documentation: **[docs/cli/installer.md](https://github.com/Falconiere/toolu/blob/main/docs/cli/installer.md)**

MIT © Falconiere Barbosa
