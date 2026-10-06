# Markdown–CLI drift gate

Skills, commands and agents tell the agent which command to run, so they are the `toolu` CLI's user interface. `cargo xtask check-markdown-cli` (#444) fails when that Markdown names a command, verb, flag, flag value or argument count the CLI lacks, runs an unknown external command, or still runs a surface a ported namespace replaced. `bun run test` and `bun run test:docs` run it; #439 moves it into `cargo xtask gate`. It reads `docs/cli/commands.json`, which the gate's `docs-cli` step proves equal to the binary, so it needs no build of `toolu` and finishes in milliseconds.

## What is scanned

- `plugins/*/skills/**/*.md`, `plugins/*/commands/*.md`, `plugins/*/agents/*.md`, `AGENTS.md` and `docs/**/*.md`.
- Not `docs/toolu/**` (dated brainstorms, specs and plans) or `docs/releases/**`: they are records, and they quote future verbs and typos on purpose.
- Fenced blocks tagged `bash`, `sh`, `shell`, `zsh`, `fish` or `console` (only its `$ ` lines). Untagged and `text` fences hold output and trees, so they are skipped; write an example of wrong usage there.
- Inline code spans that start with `toolu` and a lowercase word, a flag or a placeholder. `toolu PostToolUse dispatcher failed` is a message, not a command.

## How a `toolu` command is judged

The words after `toolu` walk the tree: commands by name or alias (hidden ones such as each plugin's `hook` verb match, but are never suggested), the command's flags plus every global flag above it, `--help`/`-h` everywhere, `--flag=value` or a value word, and listed values (`--host`). Placeholders (`<ref>`, `[<plugin>]`, `$VAR`, `${VAR}`) count as values; in a command position they try every command, and `[…]` may also be absent. `…` or `...` ends the check. An unknown command names the closest valid one; an unknown flag lists the valid ones:

```text
plugins/x/skills/x/SKILL.md:12: `toolu epic strat`: unknown command `strat` under `toolu epic` (closest: `planned`; valid: planned)
plugins/x/skills/x/SKILL.md:8: `toolu --jsn epic planned`: unknown flag `--jsn` on `toolu` (valid: --json, --quiet/-q, --host, --config-dir, --version/-V, --help/-h)
```

A fenced command must be complete: a verb where the tree needs one and every required argument. Prose may name a namespace or a command without its arguments (`toolu doctor`, `toolu hook`). Too many arguments fail everywhere. An unported namespace has only its placeholder verb, `planned`, so naming a planned verb fails until its port lands.

## Other fenced commands

Every other command in a shell fence must be a shell builtin or keyword, a function the file defines, a path (it contains `/`), a variable (`"$JEV_BUN"`), or a name on an `external` list. Leading `NAME=value` assignments, `if`/`then`/`do` and `for` headers, heredoc bodies, comments and redirections are skipped; `$(…)` and pipelines are judged command by command.

## Removed surfaces

In a plugin's Markdown, a path to a Bun bundle (`hooks/dist/<stem>.js`), a TypeScript script (`scripts/<stem>.ts`, or any `.ts`/`.js` file `bun` runs) or a stable script (`jev.sh`, `write-state.sh`, `search.sh`, `statusline.sh`) fails once the namespace that replaces it is ported: present in the tree with no `planned` verb. The stem table in `crates/xtask/src/markdown_cli/surfaces.rs` maps stems to namespaces (`plan-ledger` and `verdict` to `ledger`, `babysit-*` to `babysit`, …); any other stem maps to the namespaces the plugin owns. `AGENTS.md` and `docs/**` still describe the TypeScript tree, which stays until #440, so they are not checked for removed surfaces.

## Allowlists

`tooling/conventions/markdown-cli.json` applies to every scanned file; `tooling/conventions/markdown-cli/<plugin>.json` to the files under `plugins/<plugin>/` only. It lives outside the plugin, so it is not shipped.

```json
{
  "external": ["gh", "git"],
  "allow": [
    { "file": "AGENTS.md", "subject": "toolu install", "reason": "The Node installer's bin, until #438." }
  ]
}
```

- `external` names commands that are not `toolu`. Add a tool there when a skill starts running it.
- An `allow` entry excuses the one finding whose file and subject (the backticked text) match, and needs a reason. An entry that excuses nothing fails as stale, so an allowance cannot outlive the wrong usage it excuses. Unknown keys, a missing reason or a file outside the plugin are setup errors (exit 2).

## When a namespace is ported

1. Rewrite its Markdown from `bun …/hooks/dist/*.js` or `scripts/*.ts` to `toolu <namespace> …`; the gate fails on the old paths once the `planned` verb is gone.
2. Map any new stem in `surfaces.rs` whose namespace is not the one its plugin owns.
3. Remove allowances that went stale (for example the budget row's `render` verb of `toolu statusline` when #431 lands).

Exit codes: 0 clean, 1 findings (one `file:line: subject: problem` per line on stderr), 2 a missing or invalid tree or allowlist.
