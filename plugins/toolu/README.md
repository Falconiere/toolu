# toolu

Dual-host engineering discipline for Claude Code and Codex: workflow skills,
agents, commands, and registry-driven quality hooks. This is the core plugin;
domain plugins (`rust-quality`, `ts-quality`, …) register into it.

## Install

**Prerequisite:** install the native [`toolu` binary](../../docs/install.md) before enabling the plugin. Every hook and command runs it; Bun is not used or needed on Claude Code or Codex. See [docs/runtime.md](../../docs/runtime.md).

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
- **Runtime diagnostic** — on session start and resume, a one-line message names the native binary and version. If it is missing, enforcing hooks fail closed and the launcher says how to install the binary; see [docs/runtime.md](../../docs/runtime.md#hook-launcher).
- **docs-sync backstop** — an advisory (never a block) on push when code changes but no docs surface does.
- **Commit workflows** — Claude `/commit` and `/review-and-commit`, with Codex equivalents `$toolu:commit` and `$toolu:review-and-commit`, all reading shared workflow bodies.
- **Model routing** — Claude keeps its Haiku/Sonnet/Opus aliases. Codex defaults to Luna/medium, Terra/medium or high, and Sol/high by work class. Both are configurable under `models`.
- **Commands** — `/toolu:doctor` runs `toolu doctor` and presents the report. `/toolu:status` runs `toolu status` and presents the repository, gate and push-review state.
- **Tier-pinned agents** — Claude reads bundled definitions. The setup skill runs `toolu setup agents` to preview, install, update or remove the five Codex profiles, then `toolu doctor` and `toolu config validate`.

## Muse

Muse rejects the `agents` capability. The Muse bundle, `.muse-plugin/plugin.json`, uses one rule for all five agents: each `agents/<name>.md` is the Muse command `<name>`, and its `path` is that same file. No copy of the agent text exists, so editing an agent changes Claude and Muse together. The Codex profiles in `assets/agents/` are written separately. `cargo xtask packaging` fails when an agent has no matching command or a command points at a missing agent.

Invoke an agent on Muse as `/toolu:<name> <task>`, for example `/toolu:architect pick a queue`. A bare `/<name>` also works while no other command takes the name. A Muse plugin cannot ship agents, so the command applies the role to the current thread rather than spawning a tier-pinned subagent. The bundle also carries the six skills. The existing commands and the hooks are not in it yet (#547).

```bash
muse plugins validate plugins/toolu --json
```

## Configuration

Toggle individual skills, hooks, or MCP servers via the host-native config:
`~/.claude/toolu.config.json` / `<repo>/.claude/toolu.config.json` for Claude,
or `${CODEX_HOME:-~/.codex}/toolu.config.json` / `<repo>/.codex/toolu.config.json`
for Codex. Defaults are opt-out. Full schema: [`docs/config.md`](../../docs/config.md).
