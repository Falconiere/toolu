# toolu

Dual-host engineering discipline for Claude Code and Codex: workflow skills,
agents, commands, and registry-driven quality hooks. This is the core plugin;
domain plugins (`rust-quality`, `ts-quality`, …) register into it.

## Install

```text
/plugin install toolu@toolu
```

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add toolu@toolu
```

`toolu` has no required plugin dependencies. The `push-review` gate is reviewer-agnostic and uses the built-in `/code-review` (or `toolu-review:review`). See the root [README](../../README.md) for the full install sequence.

## What it provides

- **Supporting skills** — `orchestrator`, `debug`, and `deep-research`. The end-to-end workflow lives in the separate `delivery-flow` plugin, which depends on `toolu`, `toolu-review`, and `pr-babysit`.
- **Quality gate engine** — the `PreToolUse` / `PostToolUse` / `SessionStart` hook dispatcher and runtime **registry** that domain plugins contribute checks to (fail-closed; a module runs only while its owning plugin is installed).
- **\`push-review\` gate** — gates \`git push\` on a feature branch until the diff has been run through an accepted reviewer (the built-in \`/code-review\` skill or \`toolu-review:review\`), with a round cap that escalates instead of looping (5 rewrites against an unchanged diff; a changed diff restarts the count). Gated per target repo — `git -C <worktree> push` is judged on the worktree's own branch, diff, and state file. **Delivery is configurable** (`gates.pushReview.mode`): the shipped default advises rather than blocking; `ask` is opt-in and a yes is remembered for that diff. See [hooks/docs/gates.md](hooks/docs/gates.md).
- **docs-sync backstop** — an advisory (never a block) on push when code changes but no docs surface does.
- **Commit workflows** — Claude `/commit` and `/review-and-commit`, with Codex equivalents `$toolu:commit` and `$toolu:review-and-commit`, all reading shared workflow bodies.
- **Model routing** — Claude keeps its Haiku/Sonnet/Opus aliases. Codex defaults to Luna/medium, Terra/medium or high, and Sol/high by work class. Both are configurable under `models`.
- **Tier-pinned agents** — Claude reads bundled definitions; `$toolu:setup` safely installs the equivalent five Codex TOML profiles with preview, backup, conflict refusal, update, and removal modes.

## Configuration

Toggle individual skills, hooks, or MCP servers via the host-native config:
`~/.claude/toolu.config.json` / `<repo>/.claude/toolu.config.json` for Claude,
or `${CODEX_HOME:-~/.codex}/toolu.config.json` / `<repo>/.codex/toolu.config.json`
for Codex. Defaults are opt-out. Full schema: [`docs/config.md`](../../docs/config.md).
