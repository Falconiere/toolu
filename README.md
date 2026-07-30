<div align="center">

# toolu

### Engineering discipline, wired into your AI coding agent.

AI writes code fast — then skips the parts that keep a codebase alive: oversized files, swallowed errors, mock-only tests, undocumented exports, unreviewed pushes. **toolu** bakes that discipline back in — as hooks that gate every edit, skills that enforce a design → review → build → review → test cadence, and a plugin registry so language-specific rules ride along automatically. Runs on **Claude Code**.

[![Release](https://img.shields.io/github/v/release/Falconiere/toolu?sort=semver&color=d97757)](https://github.com/Falconiere/toolu/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Tests](https://img.shields.io/badge/tests-665%20passing-brightgreen)](#testing)
[![Hosts](https://img.shields.io/badge/host-Claude%20Code-d97757)](#install)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-blueviolet)](#contributing)

[Why](#why) · [The quality gate](#the-quality-gate) · [Install](#install) · [What's inside](#whats-inside) · [Workflow skills](#workflow-skills) · [Architecture](#architecture) · [Configuration](#configuration)

</div>

---

## Why

Your AI coding agent is a superb pair-programmer, but left alone it optimizes for *getting the change in*, not for the conventions that make a change safe to keep. You end up re-typing the same review feedback every session: *split that file, don't swallow that error, that test is all mocks, document the export, don't push that unreviewed.*

toolu moves those rules out of your head and into the tool:

- **Hooks enforce on every edit** — a post-edit quality gate checks each file the agent touches and **blocks the session from moving on while any error, warning, or test failure exists** — even in unrelated files.
- **Skills enforce a process** — an opinionated 8-phase workflow with a write/review checkpoint at every step, so design happens before code and review happens before "done."
- **A registry keeps it modular** — drop in a domain plugin (Rust rules, TypeScript rules, structural search) and its hook modules register themselves into the core engine, fail-closed, with zero wiring.

It's a personal bundle, built in the open, MIT-licensed. Take the whole thing or lift the pieces you like.

## The quality gate

The headline feature. When `rust-quality` and/or `ts-quality` is installed, every Rust/TypeScript file the agent edits is checked on the spot. Limits are **config-driven** (project/user override → the active native linter's `max-lines` → built-in default), and the gate is **multi-slot**: a failing test command and a failing file check are tracked independently, so fixing one never silently masks the other.

<table>
<tr><th align="left">TypeScript</th><th align="left">Rust</th></tr>
<tr valign="top"><td>

- File / function line limits
- No `../` relative imports — use the `@/` alias
- No `as` type assertions — use a type guard
- No hand-rolled type guards — use a Zod schema
- Tests colocated in a flat `__tests__/`
- Duplicate-type detection across the tree
- "Does too much" / too-many-factories heuristics

</td><td>

- File / function / `impl` line limits
- No `.unwrap()` / `.expect()` — use `?` or `match`
- No `unsafe` blocks
- No `#[allow]` / `#[expect]` lint suppression
- Tests in `tests/`, never inline `#[cfg(test)]`
- Flat `tests/` layout enforced

</td></tr>
</table>

The rule isn't "warn and move on" — it's a hard gate: **no new task while the gate is red.** Found a real problem? Fix it in code. (There's no "disable this check" escape hatch by design.)

## Install

toolu runs on three hosts — install it the way that matches yours.

### Claude Code

Install from the public marketplace in any Claude Code session:

```text
# 1. Add the upstream marketplaces the plugins depend on
/plugin marketplace add anthropics/claude-plugins-official
/plugin marketplace add JuliusBrussee/caveman

# 2. Add this marketplace and install the core bundle
/plugin marketplace add Falconiere/toolu
/plugin install toolu@toolu
```

Add the language gates, search, and docs tooling too:

```text
/plugin install rust-quality@toolu   # Rust quality gates
/plugin install ts-quality@toolu     # TypeScript quality gates
/plugin install ast-grep@toolu       # structural code search & rewrite
/plugin install comemory@toolu       # persistent cross-session memory
/plugin install context7@toolu       # live library documentation lookup
/plugin install exa-search@toolu     # web / code / URL search + research
```

> **Note** — `comemory`, `rust-quality`, and `ts-quality` depend on `toolu`; `ast-grep`, `context7`, and `exa-search` are standalone (zero deps). The only external-binary dependency in the bundle is `comemory` (see below). `caveman` and `code-simplifier` are **optional, recommended companions**, not required — install them only if you want caveman mode or the pre-simplify pass; when absent, `toolu` falls back (the `push-review` gate uses the built-in `/code-review`, and `code-simplifier` is invoked only if installed). Adding the marketplaces in step 1 lets Claude Code resolve those companions automatically. The `push-review` gate is **reviewer-agnostic** — it does not force you to use caveman: `caveman:cavecrew-reviewer` is preferred when present, otherwise the built-in `/code-review` skill satisfies the gate.

The `comemory` plugin wraps the standalone `comemory` binary — install it once (it is **not** on crates.io), then run setup:

```bash
brew install Falconiere/tap/comemory   # macOS + Linuxbrew (canonical)
# or the curl installer:
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Falconiere/comemory/releases/latest/download/comemory-installer.sh | sh
```

```text
/comemory:setup   # detect+guide the binary, then wire git index-code hooks, an initial index, data dir, and completions
```

The `comemory` persistent-memory mandate is **opt-in**: the `agent-memory` protocol activates only after you run `/comemory:setup` in a repo (per repo). Until then `comemory` does nothing — no memory is saved or required.

## What's inside

Ten plugins, one marketplace. Install the core alone, or add the domain plugins.

| Group | Plugin | Version | What it does |
|--------|--------|:-------:|--------------|
| Core | **`toolu`** | `1.18.0` | The registry-driven hook engine plus the 8-phase workflow skills, slash commands, the `push-review` gate, and the `deep-explore` agent. The one required plugin. |
| Quality gate | **`rust-quality`** | `0.1.0` | Rust `PostToolUse` checks — file / function / `impl` size limits, `.unwrap()`/`.expect()` bans, no `unsafe`, no lint suppression, tests in a flat `tests/`. Registers into the core engine. |
| Quality gate | **`ts-quality`** | `0.1.0` | TypeScript `PostToolUse` checks — size limits, no `../` imports, no `as` / hand-rolled guards, colocated `__tests__/`, duplicate-type detection. Registers into the core engine. |
| Code intel | **`ast-grep`** | `0.1.1` | Structural code search & rewrite (**ast-grep**) — adds the `ast-grep` skill and a `Grep → ast-grep` nudge mirrored into the registry. Standalone. |
| Code intel | **`comemory`** | `0.3.0` | Persistent cross-session **memory** + code-index search (**comemory ≥ 0.8.0**), with a `/comemory:setup` command (detect+guide the binary, then wire git index-code hooks), `delete`/`context` wrapper verbs, `PreToolUse` scope enforcement, and a `SessionStart` memory-count publisher for the statusline. |
| Knowledge | **`context7`** | `1.18.0` | Live **library documentation** & code-example lookup via the Context7 REST API. Standalone, no dependencies. |
| Knowledge | **`exa-search`** | `1.18.0` | **Web / code / URL search** plus deep research via the Exa REST API. Standalone, no dependencies. |
| Workflow | **\`toolu-review\`** | \`0.1.0\` | \`toolu-review:review\` — pre-push review mirroring the CI bot's checklist (correctness, security, perf, coverage, doc accuracy); writes the \`push-review\` state so the gate passes. Standalone. |
| Workflow | **`pr-babysit`** | `0.1.0` | `/pr-babysit:babysit` — cron-driven PR babysitter that fetches review comments + the CI review-bot verdict, triages, fixes, and chases findings to zero until CI is green. |
| UI | **`statusline`** | `0.3.1` | Optional gate-aware statusline — `model \| effort \| ctx \| gate \| folder \| branch \| mem \| caveman`, wired via a stable symlink (`/statusline:setup` to enable). Claude Code only. Standalone. |

Beyond the plugins, the core (`toolu`) also ships:

- **\`push-review\` gate** — blocks \`git push\` on a feature branch until the diff has been run through an accepted reviewer (\`caveman:cavecrew-reviewer\` when installed, the built-in \`/code-review xhigh --fix\` skill, or the \`toolu-review:review\` skill), with a round cap (5 rewrites against an unchanged diff) that escalates instead of looping forever. The state file lives under the pushed repo's own root, so `git -C <worktree> push` is gated on the worktree's branch and diff.
- **docs-sync backstop** — on `git push`, an **advisory** (never a block) when the branch diff changes code but no documentation surface (README, `docs/` guides, `SKILL.md` triggers) — a nudge to keep user-facing docs in sync with behavior. Silenced by a diff-`sha`-keyed attestation; surfaces are tunable via `docsSync.*` ([config](docs/config.md#docs-sync-surfaces-docssync)). Pairs with the "Docs in sync" convention the workflow skills enforce.
- **Slash commands** — `/commit` and `/review-and-commit`.
- **Model routing** — delegated work is tiered by its *class*, not its phrasing: mechanical → `haiku`, exploration / implementation / review → `sonnet`, synthesis / architecture → `opus`. The table is injected at session start (so it applies from turn one, and survives a compact), the workflow skills route against it, plan steps can pin their own tier (`"model": "<alias>"`, surfaced by `plan-ledger.sh status`), and every class is remappable under `models` in [config](docs/config.md#model-routing-models). Tiers are named by alias, so a new model generation needs no edits.
- **Tier-pinned agents** — `quick-task` (Haiku, mechanical), `deep-explore` / `research-agent` / `implementer` (Sonnet), `architect` (Opus, read-only design & synthesis). The cheap tiers return `ESCALATE:` instead of guessing, so routing down is safe.
- **Caveman mode** — ultra-compressed, token-frugal output (via the optional `caveman` companion).

## Workflow skills

A native, opinionated process chain. Each phase has a **write step and a review step**, so a design exists before planning and an audit happens before code is called done:

```mermaid
flowchart LR
    B(brainstorm) --> S(spec) --> SR(spec-review) --> P(plan) --> PR(plan-review) --> E(execution) --> ER(execution-review) --> T(test)
    style B fill:#d97757,color:#fff,stroke:none
    style T fill:#3fb950,color:#fff,stroke:none
    style SR fill:#1f6feb,color:#fff,stroke:none
    style PR fill:#1f6feb,color:#fff,stroke:none
    style ER fill:#1f6feb,color:#fff,stroke:none
```

- **`brainstorm`** surfaces intent, constraints, and prior art before any code.
- **`spec`** writes a design contract to `docs/toolu/specs/`; **`spec-review`** audits it.
- **`plan`** turns the spec into concrete steps; **`plan-review`** checks it's executable.
- **`execution`** drives the plan with verification checkpoints; **`execution-review`** is hard-focused on error handling.
- **`test`** enforces real-data tests (no mocks), colocated by language.

Mechanical work (renames, dep bumps, one-liners) skips the ceremony — each skill declares when *not* to fire.

The workflow skills, plus `ast-grep`, `agent-memory` (from `comemory`), `context7`, and `exa-search`, all run off the same shell hook engine.

## Architecture

Everything a plugin ships lives under its own `plugins/<name>/` directory — no symlinks, no content outside the plugin root — so a marketplace install gets the whole working tree. Domain plugins contribute hook modules to the core dispatcher through a **runtime registry**:

```mermaid
flowchart TD
    subgraph core["toolu core"]
        D["hook dispatcher<br/>PreToolUse · PostToolUse · SessionStart …"]
    end
    subgraph plugins["domain plugins"]
        RQ["rust-quality<br/>register.sh"]
        TQ["ts-quality<br/>register.sh"]
        AG["ast-grep<br/>register.sh"]
        CM["comemory<br/>register.sh"]
    end
    RQ -- "assemble concern fragments at SessionStart" --> R[("registry<br/>agent config dir/toolu/")]
    TQ -- "one assembled module per language" --> R
    AG -- "namespaced plugin__name.sh" --> R
    CM -- "namespaced plugin__name.sh" --> R
    R --> D
    D -- "runs a module only while its plugin is installed" --> OUT([enforced edit])
```

At `SessionStart`, each domain plugin's `register.sh` contributes to the registry as `<plugin-spec>__<name>.sh` — `ast-grep` and `comemory` mirror their `hooks/<event>.d/*.sh` one-to-one, while `rust-quality`/`ts-quality` assemble their ordered `hooks/concerns/` fragments into a single module per language. The core executes those copies **only while the owning plugin is installed** — uninstall the plugin and its rules vanish, fail-closed.

<details>
<summary><b>Full repository layout</b></summary>

```text
.
├── docs/                       # Runtime config schema, design notes
└── plugins/
    ├── toolu/                  # Core plugin: hook engine + process gates
    │   ├── .claude-plugin/     # plugin.json manifest
    │   ├── skills/             # brainstorm, spec(+review), plan(+review),
    │   │                       #   execution(+review), test
    │   ├── agents/             # quick-task, deep-explore, research-agent, implementer, architect
    │   ├── commands/           # commit, review-and-commit
    │   ├── hooks/              # PreToolUse / PostToolUse / SessionStart … + lib/
    │   └── settings/           # reusable settings fragments
    ├── ast-grep/               # ast-grep skill + Grep→ast-grep nudge registry module
    ├── comemory/               # agent-memory skill + scope-enforcement & memory-count registry modules
    ├── context7/               # context7 skill + Context7 REST wrapper
    ├── exa-search/             # exa-search skill + Exa REST wrapper
    ├── rust-quality/           # Rust PostToolUse quality fragments, assembled at SessionStart
    ├── ts-quality/             # TypeScript PostToolUse quality fragments, assembled at SessionStart
    ├── statusline/             # optional gate-aware statusline + SessionStart symlink hook
    ├── pr-babysit/             # /pr-babysit:babysit command + parse-verdict.sh
    └── toolu-review/            # toolu-review:review skill + push-review state writer
```

</details>

## Configuration

Toggle individual skills, hooks, or MCP servers without uninstalling anything. Use `~/.claude/toolu.config.json` (or `$CLAUDE_PROJECT_DIR/.claude/toolu.config.json`); `TOOLU_CONFIG_DIR` and `TOOLU_PROJECT_CONFIG_DIRNAME` override both roots. Defaults are opt-out — no file required.

```json
{
  "version": 1,
  "skills": { "comemory": false }
}
```

Quality-gate thresholds (file/function/impl line limits) are configurable per project and per language. Full schema and examples: [`docs/config.md`](./docs/config.md).

## Testing

The hook engine and language gates are covered by **~1340 [bats](https://github.com/bats-core/bats-core) tests**, all run in CI on every push:

```sh
bun run test              # runs lint:shell → test:shell
bats -r plugins tooling   # shell-only, fast feedback
bun run lint:shell        # shellcheck: standalone scripts + assembled concern modules
```

## Contributing

PRs and issues welcome.

1. Pick the right home — skill vs. agent vs. command vs. hook — and use the existing siblings as templates.
2. Add tests (`*.bats`, colocated in a `__tests__/`) for any hook logic.
3. Verify in a real session before committing.
4. Use a [Conventional Commits](https://www.conventionalcommits.org/) subject (`feat(skills): add foo`).

## Releases

Releases are automated with [release-please](https://github.com/googleapis/release-please) — you do **not** bump versions or tag by hand. The repo ships as **one version**: `toolu` and every plugin share the same `vX.Y.Z`.

- Merge Conventional Commits to `main`. release-please maintains one batched **Release PR** for the whole repo. **Any** path counts — a `feat` in `tooling/` or `.github/` releases just like one under `plugins/`.
- `feat` / `fix` / `feat!` (or `BREAKING CHANGE`) drive minor / patch / major bumps; `chore` / `docs` / `ci` / `refactor` ship no release.
- Review the Release PR, then **merge it to cut the release** — the only manual step. It bumps `package.json` and every plugin's `plugin.json` to the new version, updates `CHANGELOG.md`, tags `vX.Y.Z`, and publishes the GitHub Release. The marketplace re-extracts a plugin when its `plugin.json` version changes.

`tooling/release.sh` is **deprecated**, kept only as a manual escape hatch for when the automation is unavailable.

## References

- [Claude Code docs](https://docs.claude.com/en/docs/claude-code) ·
  [Skills](https://docs.claude.com/en/docs/claude-code/skills) ·
  [Subagents](https://docs.claude.com/en/docs/claude-code/sub-agents) ·
  [Slash commands](https://docs.claude.com/en/docs/claude-code/slash-commands) ·
  [Hooks](https://docs.claude.com/en/docs/claude-code/hooks) ·
  [Plugins](https://docs.claude.com/en/docs/claude-code/plugins)

## License

[MIT](./LICENSE) © [Falconiere Barbosa](https://github.com/falconiere)
