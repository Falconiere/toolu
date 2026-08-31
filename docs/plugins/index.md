# toolu Plugin Documentation

Each page covers what the plugin does, how to install it, its hooks/skills/commands, configuration, and real-world usage examples.

## Plugin Index

| # | Plugin | Type | Depends On | Quick Summary |
|:--:|--------|------|:----------:|---------------|
| 1 | [**toolu**](../toolu/README.md) | Core | — | Dual-host hook engine + workflow + push-review gate + agent routing |
| 2 | [**ast-grep**](../ast-grep/README.md) | Code Intel | — | Structural code search & rewrite (tree-sitter AST patterns) |
| 3 | [**toolu-review**](../toolu-review/README.md) | Workflow | — | Pre-push review mirroring the CI review bot's checklist |
| 4 | [**comemory**](../comemory/README.md) | Code Intel | `toolu` | Persistent cross-session agent memory + project skills + code-index search |
| 5 | [**context7**](../context7/README.md) | Knowledge | — | Live library documentation & code-example lookup |
| 6 | [**exa-search**](../exa-search/README.md) | Knowledge | — | Web / code / URL search plus deep research |
| 7 | [**jira**](../jira/README.md) | Workflow | — | Jira issue search & workflow from the session |
| 8 | [**pr-babysit**](../pr-babysit/README.md) | Workflow | `toolu` | Claude cron / durable Codex PR babysitter that chases findings to zero |
| 9 | [**python-quality**](../python-quality/README.md) | Quality Gate | `toolu` | Python post-edit quality checks (size, suppression, test layout, no-mocks) |
| 10 | [**rust-quality**](../rust-quality/README.md) | Quality Gate | `toolu` | Rust post-edit quality checks (size, unsafe, unwrap bans) |
| 11 | [**statusline**](../statusline/README.md) | Status | — | Persistent Claude statusline plus explicit Codex repository/gate status |
| 12 | [**ts-quality**](../ts-quality/README.md) | Quality Gate | `toolu` | TypeScript post-edit quality checks (size, imports, type guards) |
| 13 | [**agent-browser**](../../plugins/agent-browser/README.md) | Browser | — | Token-lean live browser automation via accessibility-tree snapshots |

`git-better` (token-lean `gb` reads + cached repo-convention detection) is bundled as a skill inside `toolu` core — no separate plugin/install/marketplace row.

## Architecture Overview

The **toolu core** (`toolu` plugin) is the hub: it provides the hook dispatcher (`PreToolUse`, `PostToolUse`, `SessionStart`, …) and a runtime registry. Domain plugins (`rust-quality`, `ts-quality`, `python-quality`, `comemory`, `ast-grep`) contribute hook modules to that registry — each module runs only while its owning plugin is installed, fail-closed.

```
toolu core (hook dispatcher + registry)
  ├── rust-quality    ──→ PostToolUse checks on Rust files
  ├── ts-quality      ──→ PostToolUse checks on TS files
  ├── python-quality  ──→ PostToolUse checks on Python files
  ├── comemory        ──→ PreToolUse scope enforcement + SessionStart memory count + project-skills index/curator
  └── ast-grep        ──→ PreToolUse Grep→ast-grep nudge + PostToolUse byte-savings
```

Standalone plugins (no `toolu` dependency) work independently via their own skills and commands.

## Shared Configuration

All plugins share the same host-native config: `~/.claude/toolu.config.json`
and `<repo>/.claude/toolu.config.json` on Claude Code, or
`${CODEX_HOME:-~/.codex}/toolu.config.json` and
`<repo>/.codex/toolu.config.json` on Codex. Toggle skills, hooks, or MCP
servers without uninstalling:

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
/plugin install python-quality@toolu
/plugin install ast-grep@toolu
/plugin install comemory@toolu
/plugin install context7@toolu
/plugin install exa-search@toolu
/plugin install jira@toolu
/plugin install toolu-review@toolu
/plugin install pr-babysit@toolu
/plugin install statusline@toolu
/plugin install agent-browser@toolu
```

For Codex, install the core first, then the optional plugins:

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add toolu@toolu
codex plugin add rust-quality@toolu
codex plugin add ts-quality@toolu
codex plugin add python-quality@toolu
# Repeat `codex plugin add <name>@toolu` for any other plugin above.
```

Codex support covers CLI, IDE extension, and ChatGPT desktop Codex on macOS
and Linux. Codex cloud and Windows are limitations for this release. Review
and trust plugin hooks through `/hooks` before they execute.
