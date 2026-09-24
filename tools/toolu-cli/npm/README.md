# @toolu/plugins

Install [toolu](https://github.com/Falconiere/toolu) plugins into Claude Code and Codex.

```bash
npx @toolu/plugins install              # every catalog plugin, core first
npx @toolu/plugins install rust-quality # one plugin and its dependency
npx @toolu/plugins list                 # catalog joined with what is installed
npx @toolu/plugins remove jira --yes
npx @toolu/plugins update
```

A global install (`npm install -g @toolu/plugins`) puts the same commands on your `PATH` as `toolu install`, `toolu list`, and so on. This package replaces `@toolu/cli`; if you installed that globally, run `npm uninstall -g @toolu/cli` first, because both provide the `toolu` command.

## What it does

It drives each host's own plugin CLI — `claude plugin install`, `codex plugin add` — rather than writing into their directories itself. Install order comes from the marketplace catalog's dependency edges, so `rust-quality` pulls in `toolu` first.

Already-installed plugins are reported and left alone. If the installed version differs from the one the marketplace offers, both are named and nothing changes; `update` is how you move it.

## Options

| Flag | Meaning |
|------|---------|
| `--host <id>` | `claude`, `codex`, or `opencode`. Detected when omitted. |
| `--scope <scope>` | `user`, `project`, `local`. **Claude Code only.** |
| `--config <path>` | Replay a `.toolu/plugins.json` selection. |
| `--yes`, `-y` | Confirm a destructive or command-declaring operation. |
| `--dry-run` | Print the host commands in order; run none. |
| `--no-input` | Never prompt; fail listing what is missing. For CI. |
| `--json` | Machine-readable output where supported. |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Succeeded, or already satisfied |
| `1` | Something failed; every failure is named on stderr |
| `2` | Usage error: unknown command, flag, host, or plugin |
| `3` | Required input missing, or an ambiguous host with no `--host` |
| `130` | An interactive prompt was cancelled |

## Not yet supported

`--host opencode` exits `2`. OpenCode has its own plugin CLI, but it installs npm packages rather than marketplace entries, so the bash plugins are not addressable through it. Install the bridge directly instead:

```bash
opencode plugin add @toolu/opencode
```

Full documentation: **[docs/cli.md](https://github.com/Falconiere/toolu/blob/main/docs/cli.md)**

MIT © Falconiere Barbosa
