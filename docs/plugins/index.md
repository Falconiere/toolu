# toolu Plugin Documentation

Each page covers what the plugin does, how to install it, its hooks/skills/commands, configuration, and real-world usage examples.

## Plugin Index

| # | Plugin | Type | Depends On | Quick Summary |
|:--:|--------|------|:----------:|---------------|
| 1 | [**toolu**](../toolu/README.md) | Core | — | Hook engine for Claude Code and Codex (Muse in progress), push-review gate, and agent routing |
| 2 | [**ast-grep**](../ast-grep/README.md) | Code Intel | — | Structural code search & rewrite (tree-sitter AST patterns) |
| 3 | [**toolu-review**](../toolu-review/README.md) | Workflow | — | Pre-push review mirroring CI `code-review@v8` (Jev-enabled) |
| 4 | [**delivery-flow**](../../plugins/delivery-flow/README.md) | Workflow | `toolu`, `toolu-review`, `pr-babysit`, `brainstorm` | One skill for brainstorm through checked PR delivery |
| 5 | [**pr-babysit**](../pr-babysit/README.md) | Workflow | `toolu` | Claude cron / durable Codex PR babysitter that chases findings to zero |
| 6 | [**epic-orchestrator**](../epic-orchestrator/README.md) | Workflow | `toolu`, `delivery-flow`, `pr-babysit` | Drive a GitHub, Jira, or Linear epic to merged PRs via herdr workers on any host and a merge gate |
| 7 | [**python-quality**](../python-quality/README.md) | Quality Gate | `toolu` | Python post-edit quality checks (size, suppression, test layout, no-mocks) |
| 8 | [**rust-quality**](../rust-quality/README.md) | Quality Gate | `toolu` | Rust post-edit quality checks (size, unsafe, unwrap bans) |
| 9 | [**statusline**](../statusline/README.md) | Status | — | Persistent Claude statusline plus explicit Codex repository/gate status |
| 10 | [**ts-quality**](../ts-quality/README.md) | Quality Gate | `toolu` | TypeScript post-edit quality checks (size, imports, type guards) |
| 11 | [**jev**](../jev/README.md) | Knowledge | — | Typed judgments from TypeSafe's Jev — probability, choice, and score answers code can branch on |
| 12 | [**brainstorm**](../../plugins/brainstorm/README.md) | Workflow | — | Evidence-backed design triage and a recommended approach; standalone and delivery-flow phase 1 |
| 13 | [**code-review**](../../plugins/code-review/README.md) | Workflow | — | Native local review of the push diff with OpenRouter and Jev (epic #660); `toolu code-review run` fails closed until the pipeline lands |
| 14 | [**sandbox**](../../plugins/sandbox/README.md) | Environment | — | A Linux place for builds, gates and agent CLIs: a Lima VM on macOS, the host on Linux; `toolu sandbox run` runs a command there |
| 15 | [**git-better**](../../plugins/git-better/README.md) | Workflow | `toolu` | Semantic branches and commits agents can read: scoped Conventional Commit subjects with an `Intent:` trailer and `<type>/<ref>-<slug>` branch names, checked before git runs |
| 16 | [**epic-creator**](../../plugins/epic-creator/README.md) | Workflow | `brainstorm` | Turn an idea into a GitHub epic the engine can run: one-PR sub-issues with cited decisions and runnable checks, created after one confirmation |

## Architecture Overview


```
toolu core (hook dispatcher + registry)
  ├── rust-quality    ──→ PostToolUse checks on Rust files
  ├── ts-quality      ──→ PostToolUse checks on TS files
  ├── python-quality  ──→ PostToolUse checks on Python files
  └── ast-grep        ──→ PreToolUse Grep→ast-grep nudge + PostToolUse byte-savings
```

Standalone plugins (no `toolu` dependency) work independently via their own skills and commands. `delivery-flow` installs its declared dependencies through Claude Code or the toolu CLI; native Codex plugin installs require adding those dependencies explicitly.

Hook entries and registry contributions run the native `toolu` binary through the generated launcher; Bun is not used ([runtime.md](../runtime.md)).

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
```bash
# Optional: lets Claude Code resolve the code-simplifier companion. Nothing is
# installed from it; skip if you do not want the pre-simplify pass.
claude plugin marketplace add anthropics/claude-plugins-official

# Adds the toolu marketplace and installs every catalog plugin, core first.
# Already-installed plugins are reported and left alone.
toolu plugins install
```
<!-- /install-everything:claude -->

#### Codex

<!-- install-everything:codex -->
```bash
# Adds the toolu marketplace and installs every catalog plugin, core first.
toolu plugins install --host codex
```

After they are installed, review and trust the hooks in `/hooks` before they run.
<!-- /install-everything:codex -->

Codex support covers CLI, IDE extension, and ChatGPT desktop Codex on macOS
and Linux. Codex cloud and Windows are limitations for this release. Review
and trust plugin hooks through `/hooks` before they execute.
