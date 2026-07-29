# toolu

Falconiere's personal Claude Code bundle: skills, agents, slash commands, and hooks. The core plugin — the registry-driven hook engine plus the 8-phase workflow, the `push-review` gate, and the `deep-explore` agent. The one required plugin; the domain plugins (`rust-quality`, `ts-quality`, `comemory`, …) register into it.

## Install

```
/plugin install toolu@toolu
```

`toolu` has no required plugin dependencies; `comemory` is the bundle's sole external-binary dependency. `code-simplifier` (from the `claude-plugins-official` marketplace) and `caveman` (from the `caveman` marketplace) are **optional, recommended companions** — install them only if you want the pre-simplify pass or caveman mode. When they are absent `toolu` falls back: the `push-review` gate uses the built-in `/code-review`, and `code-simplifier` is invoked only if installed. Adding those two marketplaces first lets Claude Code resolve the optional companions automatically. See the root [README](../../README.md) for the full install sequence.

## What it provides

- **Workflow skills** — an 8-phase chain with a write step and a review step each: `brainstorm`, `spec`, `spec-review`, `plan`, `plan-review`, `execution`, `execution-review`, `test`. Plus two non-chain skills: `orchestrator` for delegating broad work across subagents, and `debug` — a break-glass scientific-debug loop (reproduce → observe → hypothesize → isolate root cause → fix → regress) any phase can drop into, backed by language-agnostic evidence helpers (`debug-testfail`/`debug-stack`/`debug-log`) and an opt-in Sentry adapter.
- **Quality gate engine** — the `PreToolUse` / `PostToolUse` / `SessionStart` hook dispatcher and runtime **registry** that domain plugins contribute checks to (fail-closed; a module runs only while its owning plugin is installed).
- **\`push-review\` gate** — blocks \`git push\` on a feature branch until the diff has been run through an accepted reviewer (\`caveman:cavecrew-reviewer\`, the built-in \`/code-review\` skill, or \`toolu-review:review\`), with a round cap that escalates instead of looping (5 rewrites against an unchanged diff; a changed diff restarts the count). Gated per target repo — `git -C <worktree> push` is judged on the worktree's own branch, diff, and state file.
- **docs-sync backstop** — an advisory (never a block) on push when code changes but no docs surface does.
- **Slash commands** — `/commit` and `/review-and-commit`.
- **Model routing** — delegated work is tiered by its class (mechanical → `haiku`; exploration / implementation / review → `sonnet`; synthesis / architecture → `opus`), injected at session start, honored by the workflow skills, pinnable per plan step (`"model": "<alias>"`), and remappable under `models` in the config.
- **Tier-pinned agents** — `quick-task` (Haiku), `deep-explore` / `research-agent` / `implementer` (Sonnet), `architect` (Opus, read-only). Structural exploration runs on ast-grep; the cheap tiers escalate rather than guess.

## Configuration

Toggle individual skills, hooks, or MCP servers via `~/.claude/toolu.config.json` (or `$CLAUDE_PROJECT_DIR/.claude/toolu.config.json`); defaults are opt-out, no file required. Quality-gate thresholds are configurable per project and language. Full schema: [`docs/config.md`](../../docs/config.md).
