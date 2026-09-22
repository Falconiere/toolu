# toolu Plugin Documentation

Each page covers what the plugin does, how to install it, its hooks/skills/commands, configuration, and real-world usage examples.

## Plugin Index

| # | Plugin | Type | Depends On | Quick Summary |
|:--:|--------|------|:----------:|---------------|
| 1 | [**toolu**](../toolu/README.md) | Core | — | Dual-host hook engine + workflow + push-review gate + agent routing |
| 2 | [**ast-grep**](../ast-grep/README.md) | Code Intel | — | Structural code search & rewrite (tree-sitter AST patterns) |
| 3 | [**toolu-review**](../toolu-review/README.md) | Workflow | — | Pre-push review mirroring CI `code-review@v8` (Jev-enabled) |
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
```bash
# Optional: lets Claude Code resolve the code-simplifier companion. Nothing is
# installed from it; skip if you do not want the pre-simplify pass.
claude plugin marketplace add anthropics/claude-plugins-official

# Adds the toolu marketplace and installs every catalog plugin, core first.
# Already-installed plugins are reported and left alone.
npx toolu plugins install
```
<!-- /install-everything:claude -->

#### Codex

<!-- install-everything:codex -->
```bash
# Adds the toolu marketplace and installs every catalog plugin, core first.
npx toolu plugins install --host codex
```

After they are installed, review and trust the hooks in `/hooks` before they run.
<!-- /install-everything:codex -->

#### OpenCode

OpenCode has no marketplace install yet — paste this prompt so the agent follows the git-clone + Bun wiring in [docs/opencode.md](../opencode.md). Phase-1 enables the `toolu` bash plugin only.

<!-- install-everything:opencode -->
```text
Install toolu for OpenCode in this project (git-clone + Bun; no marketplace). Skip steps already done.

1. Clone https://github.com/Falconiere/toolu.git and check out the latest release tag (vX.Y.Z). In that clone run: bun install --frozen-lockfile
2. export TOOLU_REPO_ROOT=/absolute/path/to/that/clone  (TOOLU_ROOT is an alias)
3. In THIS project create:
   - .opencode/package.json with dependencies "@toolu/opencode": "file:$TOOLU_REPO_ROOT/tools/toolu-opencode", "@toolu/core": "file:$TOOLU_REPO_ROOT/packages/toolu-core", "@opencode/plugin": "2.0.12" (adjust file: paths to your layout)
   - .opencode/plugins/toolu.ts containing: export { default } from "@toolu/opencode/plugin";
   - .opencode/toolu/plugins.json containing: { "version": 1, "enabled": ["toolu"] }
   - optional .opencode/toolu.config.json for gates (same schema as other hosts)
4. In opencode.json (or .opencode/opencode.json), set skills.paths to include $TOOLU_REPO_ROOT/tools/toolu-opencode/generated/skills so the generated surface is discoverable
5. Restart OpenCode. Smoke-check: bun run test:conformance in the toolu clone, or attempt a protected .env edit with protectedFiles mode block and confirm deny before bytes change

Do not install comemory via toolu. Full detail: docs/opencode.md
```
<!-- /install-everything:opencode -->

Codex support covers CLI, IDE extension, and ChatGPT desktop Codex on macOS
and Linux. Codex cloud and Windows are limitations for this release. Review
and trust plugin hooks through `/hooks` before they execute.
