# @toolu/plugins

Install [toolu](https://github.com/Falconiere/toolu) plugins into Claude Code, Codex and OpenCode.

```bash
npx @toolu/plugins install              # every catalog plugin, core first
npx @toolu/plugins install rust-quality # one plugin and its dependency
npx @toolu/plugins list                 # catalog joined with what is installed
npx @toolu/plugins remove ast-grep --yes
npx @toolu/plugins update
```

A global install (`npm install -g @toolu/plugins`) puts the same commands on your `PATH` as `toolu install`, `toolu list`, and so on. This package replaces `@toolu/cli`; if you installed that globally, run `npm uninstall -g @toolu/cli` first, because both provide the `toolu` command.

## What it does

It drives each host's own plugin CLI — `claude plugin install`, `codex plugin add` — rather than writing into their directories itself. Install order comes from the marketplace catalog's dependency edges, so `rust-quality` pulls in `toolu` first.

OpenCode has no plugin management commands, so `--host opencode` edits its documented config instead: it adds `@toolu/opencode` to the `plugin` array of the global config (or the project's, with `--scope project`) and records named plugins in a `toolu/plugins.json` selection. Comments, other keys and other plugins in those files are kept.

Already-installed plugins are reported and left alone. If the installed version differs from the one the marketplace offers, both are named and nothing changes; `update` is how you move it.

On a TTY (without `--no-input`), several hosts on `PATH` prompts instead of failing: `install` multi-selects hosts and, when no names were given, multi-selects plugins (default: all). Other verbs single-select one host. Cancel exits `130`.

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, or `opencode`. Detected when omitted. |
| `--scope <scope>` | Claude Code: `user`, `project`, `local`. OpenCode: `user` (default) or `project`. Not for Codex. |
| `--config <path>` | Replay a `.toolu/plugins.json` selection. |
| `--yes`, `-y` | Confirm a destructive or command-declaring operation. |
| `--dry-run` | Print the host commands (OpenCode: the planned config edits); change nothing. |
| `--no-input` | Never prompt; fail listing what is missing. For CI. |
| `--json` | Machine-readable output where supported. |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Succeeded, or already satisfied |
| `1` | Something failed; every failure is named on stderr |
| `2` | Usage error: unknown command, flag, host, or plugin |
| `3` | Required input missing, or an ambiguous host with no TTY / `--no-input` |
| `130` | An interactive prompt was cancelled |

Full documentation: **[docs/cli.md](https://github.com/Falconiere/toolu/blob/main/docs/cli.md)**

MIT © Falconiere Barbosa
