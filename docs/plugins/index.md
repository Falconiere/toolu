# toolu Plugin Documentation

Each page covers what the plugin does, how to install it, its hooks/skills/commands, configuration, and real-world usage examples.

## Plugin Index

| # | Plugin | Type | Depends On | Quick Summary |
|:--:|--------|------|:----------:|---------------|
| 1 | [**toolu**](../toolu/README.md) | Core | `code-simplifier`, `caveman` | Hook engine + 8-phase workflow + push-review gate + deep-explore agent |
| 2 | [**ast-grep**](../ast-grep/README.md) | Code Intel | — | Structural code search & rewrite (tree-sitter AST patterns) |
| 3 | [**toolu-review**](../toolu-review/README.md) | Workflow | — | Pre-push review mirroring the CI review bot's checklist |
| 4 | [**comemory**](../comemory/README.md) | Code Intel | `toolu` | Persistent cross-session agent memory + code-index search |
| 5 | [**context7**](../context7/README.md) | Knowledge | — | Live library documentation & code-example lookup |
| 6 | [**exa-search**](../exa-search/README.md) | Knowledge | — | Web / code / URL search plus deep research |
| 7 | [**git-better**](../git-better/README.md) | Workflow | — | Token-lean `gb` wrapper with repo-convention detection |
| 8 | [**jira**](../jira/README.md) | Workflow | — | Jira issue search & workflow from the session |
| 9 | [**pr-babysit**](../pr-babysit/README.md) | Workflow | `toolu` | Cron-driven PR babysitter that chases review findings to zero |
| 10 | [**rust-quality**](../rust-quality/README.md) | Quality Gate | `toolu` | Rust post-edit quality checks (size, unsafe, unwrap bans) |
| 11 | [**stats**](../stats/README.md) | UI | — | Measured Claude Code usage report (tokens, cost, cache-hit %) |
| 12 | [**statusline**](../statusline/README.md) | UI | — | Gate-aware terminal statusline (model, effort, ctx, usage, gate) |
| 13 | [**ts-quality**](../ts-quality/README.md) | Quality Gate | `toolu` | TypeScript post-edit quality checks (size, imports, type guards) |

## Architecture Overview

The **toolu core** (`toolu` plugin) is the hub: it provides the hook dispatcher (`PreToolUse`, `PostToolUse`, `SessionStart`, …) and a runtime registry. Domain plugins (`rust-quality`, `ts-quality`, `comemory`, `ast-grep`, `git-better`) contribute hook modules to that registry — each module runs only while its owning plugin is installed, fail-closed.

```
toolu core (hook dispatcher + registry)
  ├── rust-quality  ──→ PostToolUse checks on Rust files
  ├── ts-quality    ──→ PostToolUse checks on TS files
  ├── comemory      ──→ PreToolUse scope enforcement + SessionStart memory count
  ├── ast-grep      ──→ PreToolUse Grep→ast-grep nudge + PostToolUse byte-savings
  └── git-better    ──→ PreToolUse git→gb nudge + PostToolUse byte-savings
```

Standalone plugins (no `toolu` dependency) work independently via their own skills and commands.

## Shared Configuration

All plugins (core + domain) share the same config file at `~/.claude/toolu.config.json` (Claude Code) or `~/.pi/agent/toolu.config.json` (pi). Toggle skills, hooks, or MCP servers without uninstalling:

```json
{
  "version": 1,
  "skills": { "comemory": false },
  "hooks":  { "user-prompt-submit": false },
  "mcp":    { "figma": false }
}
```

See [`config.md`](../config.md) for the full schema.

## Quick Start — Install Everything

```text
# 1. Add marketplaces
/plugin marketplace add anthropics/claude-plugins-official
/plugin marketplace add JuliusBrussee/caveman
/plugin marketplace add Falconiere/toolu

# 2. Install core
/plugin install toolu@toolu

# 3. Install domain plugins
/plugin install rust-quality@toolu
/plugin install ts-quality@toolu
/plugin install ast-grep@toolu
/plugin install comemory@toolu
/plugin install context7@toolu
/plugin install exa-search@toolu
/plugin install git-better@toolu
/plugin install jira@toolu
/plugin install toolu-review@toolu
/plugin install pr-babysit@toolu
/plugin install stats@toolu
/plugin install statusline@toolu
```

For pi users:

```bash
pi install https://github.com/Falconiere/toolu
```
