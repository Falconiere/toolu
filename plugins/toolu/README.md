# toolu

Falconiere's personal Claude Code bundle: skills, agents, slash commands, and hooks. The core plugin — the registry-driven hook engine plus the 8-phase workflow, the `push-review` gate, and the `deep-explore` agent. The one required plugin; the domain plugins (`rust-quality`, `ts-quality`, `comemory`, …) register into it.

## Install

```
/plugin install toolu@toolu
```

Depends on `code-simplifier` (from the `claude-plugins-official` marketplace) and `caveman` (from the `caveman` marketplace) — add those marketplaces first so Claude Code can resolve them. See the root [README](../../README.md) for the full install sequence.

## What it provides

- **Workflow skills** — an 8-phase chain with a write step and a review step each: `brainstorm`, `spec`, `spec-review`, `plan`, `plan-review`, `execution`, `execution-review`, `test`. Plus an `orchestrator` skill for delegating broad work across subagents.
- **Quality gate engine** — the `PreToolUse` / `PostToolUse` / `SessionStart` hook dispatcher and runtime **registry** that domain plugins contribute checks to (fail-closed; a module runs only while its owning plugin is installed).
- **`push-review` gate** — blocks `git push` on a feature branch until the diff has been run through an accepted reviewer (`caveman:cavecrew-reviewer`, the built-in `/code-review` skill, or `code-review:review`), with a round cap that escalates instead of looping.
- **docs-sync backstop** — an advisory (never a block) on push when code changes but no docs surface does.
- **Slash commands** — `/commit` and `/review-and-commit`.
- **`deep-explore` agent** — structural codebase exploration via ast-grep.

## Configuration

Toggle individual skills, hooks, or MCP servers via `~/.claude/toolu.config.json` (or `$CLAUDE_PROJECT_DIR/.claude/toolu.config.json`); defaults are opt-out, no file required. Quality-gate thresholds are configurable per project and language. Full schema: [`docs/config.md`](../../docs/config.md).
