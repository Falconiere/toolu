# toolu

Dual-host engineering discipline for Claude Code and Codex: workflow skills,
agents, commands, and registry-driven quality hooks. This is the core plugin;
domain plugins (`rust-quality`, `ts-quality`, `comemory`, …) register into it.

## Install

```text
/plugin install toolu@toolu
```

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add toolu@toolu
```

`toolu` has no required plugin dependencies; `comemory` is the bundle's sole external-binary dependency. `code-simplifier` (from the `claude-plugins-official` marketplace) and `caveman` (from the `caveman` marketplace) are **optional, recommended companions** — install them only if you want the pre-simplify pass or caveman mode. When they are absent `toolu` falls back: the `push-review` gate uses the built-in `/code-review`, and `code-simplifier` is invoked only if installed. Adding those two marketplaces first lets Claude Code resolve the optional companions automatically. See the root [README](../../README.md) for the full install sequence.

## What it provides

- **Workflow skills** — an 8-phase chain with a write step and a review step each: `brainstorm`, `spec`, `spec-review`, `plan`, `plan-review`, `execution`, `execution-review`, `test`. Plus three non-chain skills: `orchestrator` for deciding whether a large task should be split at all and delegating what genuinely benefits, `debug` — a break-glass scientific-debug loop (reproduce → observe → hypothesize → isolate root cause → fix → regress) any phase can drop into, backed by language-agnostic evidence helpers (`debug-testfail`/`debug-stack`/`debug-log`) and an opt-in Sentry adapter, and `deep-research` — a research pipeline: guiding questions fanned out across `research-agent` workers (exa-search + context7), verified, and delivered as a cited report under `docs/research/`. Skills pick recommended defaults and proceed; they do not prompt.
- **Quality gate engine** — the `PreToolUse` / `PostToolUse` / `SessionStart` hook dispatcher and runtime **registry** that domain plugins contribute checks to (fail-closed; a module runs only while its owning plugin is installed).
- **\`push-review\` gate** — gates \`git push\` on a feature branch until the diff has been run through an accepted reviewer (\`caveman:cavecrew-reviewer\`, the built-in \`/code-review\` skill, or \`toolu-review:review\`), with a round cap that escalates instead of looping (5 rewrites against an unchanged diff; a changed diff restarts the count). Gated per target repo — `git -C <worktree> push` is judged on the worktree's own branch, diff, and state file. **Delivery is configurable** (`gates.pushReview.mode`): the shipped default advises rather than blocking; `ask` is opt-in and a yes is remembered for that diff. See [hooks/docs/gates.md](hooks/docs/gates.md).
- **docs-sync backstop** — an advisory (never a block) on push when code changes but no docs surface does.
- **Commit workflows** — Claude `/commit` and `/review-and-commit`, with Codex equivalents `$toolu:commit` and `$toolu:review-and-commit`, all reading shared workflow bodies.
- **Model routing** — Claude keeps its Haiku/Sonnet/Opus aliases. Codex defaults to Luna/medium, Terra/medium or high, and Sol/high by work class. Both are configurable under `models`.
- **Tier-pinned agents** — Claude reads bundled definitions; `$toolu:setup` safely installs the equivalent five Codex TOML profiles with preview, backup, conflict refusal, update, and removal modes.

## Configuration

Toggle individual skills, hooks, or MCP servers via the host-native config:
`~/.claude/toolu.config.json` / `<repo>/.claude/toolu.config.json` for Claude,
or `${CODEX_HOME:-~/.codex}/toolu.config.json` / `<repo>/.codex/toolu.config.json`
for Codex. Defaults are opt-out. Full schema: [`docs/config.md`](../../docs/config.md).
