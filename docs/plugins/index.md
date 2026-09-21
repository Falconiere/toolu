# toolu Plugin Documentation

Each page covers what the plugin does, how to install it, its hooks/skills/commands, configuration, and real-world usage examples.

## Plugin Index

| # | Plugin | Type | Depends On | Quick Summary |
|:--:|--------|------|:----------:|---------------|
| 1 | [**toolu**](../toolu/README.md) | Core | — | Dual-host hook engine + workflow + push-review gate + agent routing |
| 2 | [**ast-grep**](../ast-grep/README.md) | Code Intel | — | Structural code search & rewrite (tree-sitter AST patterns) |
| 3 | [**toolu-review**](../toolu-review/README.md) | Workflow | — | Pre-push review mirroring the CI review bot's checklist |
| 4 | [**context7**](../context7/README.md) | Knowledge | — | Live library documentation & code-example lookup |
| 5 | [**exa-search**](../exa-search/README.md) | Knowledge | — | Web / code / URL search plus deep research |
| 6 | [**jira**](../jira/README.md) | Workflow | — | Jira issue search & workflow from the session |
| 7 | [**pr-babysit**](../pr-babysit/README.md) | Workflow | `toolu` | Claude cron / durable Codex PR babysitter that chases findings to zero |
| 8 | [**python-quality**](../python-quality/README.md) | Quality Gate | `toolu` | Python post-edit quality checks (size, suppression, test layout, no-mocks) |
| 9 | [**rust-quality**](../rust-quality/README.md) | Quality Gate | `toolu` | Rust post-edit quality checks (size, unsafe, unwrap bans) |
| 10 | [**statusline**](../statusline/README.md) | Status | — | Persistent Claude statusline plus explicit Codex repository/gate status |
| 11 | [**ts-quality**](../ts-quality/README.md) | Quality Gate | `toolu` | TypeScript post-edit quality checks (size, imports, type guards) |
| 12 | [**agent-browser**](../../plugins/agent-browser/README.md) | Browser | — | Token-lean live browser automation via accessibility-tree snapshots |
| 13 | [**jev**](../jev/README.md) | Knowledge | — | Typed judgments from TypeSafe's Jev — probability, choice, and score answers code can branch on |

## Architecture Overview


```
toolu core (hook dispatcher + registry)
  ├── rust-quality    ──→ PostToolUse checks on Rust files
  ├── ts-quality      ──→ PostToolUse checks on TS files
  ├── python-quality  ──→ PostToolUse checks on Python files
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
  "hooks":  { "user-prompt-submit": false },
  "mcp":    { "figma": false }
}
```

See [`config.md`](../config.md) for the full schema.

## Quick Start — Install Everything

Paste one prompt into the host. It adds the marketplace and installs every
plugin in the catalog, core first. Do not install comemory. The same prompts
are in the root [README](../../README.md#install-everything).

#### Claude Code

<!-- install-everything:claude -->
```text
Install every toolu plugin for Claude Code at user scope. Run these commands in a terminal, in order. Skip a command that reports the marketplace or plugin is already installed.

claude plugin marketplace add anthropics/claude-plugins-official
claude plugin marketplace add Falconiere/toolu
claude plugin install toolu@toolu --scope user
claude plugin install agent-browser@toolu --scope user
claude plugin install ast-grep@toolu --scope user
claude plugin install context7@toolu --scope user
claude plugin install exa-search@toolu --scope user
claude plugin install jev@toolu --scope user
claude plugin install jira@toolu --scope user
claude plugin install pr-babysit@toolu --scope user
claude plugin install python-quality@toolu --scope user
claude plugin install rust-quality@toolu --scope user
claude plugin install statusline@toolu --scope user
claude plugin install toolu-review@toolu --scope user
claude plugin install ts-quality@toolu --scope user
```
<!-- /install-everything:claude -->

#### Codex

<!-- install-everything:codex -->
```text
Install every toolu plugin for Codex. Run these commands in a terminal, in order. Skip a command that reports the marketplace or plugin is already installed. After they are installed, review and trust the hooks in /hooks before they run.

codex plugin marketplace add Falconiere/toolu
codex plugin add toolu@toolu
codex plugin add agent-browser@toolu
codex plugin add ast-grep@toolu
codex plugin add context7@toolu
codex plugin add exa-search@toolu
codex plugin add jev@toolu
codex plugin add jira@toolu
codex plugin add pr-babysit@toolu
codex plugin add python-quality@toolu
codex plugin add rust-quality@toolu
codex plugin add statusline@toolu
codex plugin add toolu-review@toolu
codex plugin add ts-quality@toolu
```
<!-- /install-everything:codex -->

`code-simplifier` is an optional companion from `claude-plugins-official`. The
Claude prompt adds that marketplace and does not install the companion.

Codex support covers CLI, IDE extension, and ChatGPT desktop Codex on macOS
and Linux. Codex cloud and Windows are limitations for this release. Review
and trust plugin hooks through `/hooks` before they execute.
