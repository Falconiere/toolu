<div align="center">

# toolu

### Engineering discipline, wired into your AI coding agent.

AI writes code fast — then skips the parts that keep a codebase alive: oversized files, swallowed errors, mock-only tests, undocumented exports, unreviewed pushes. **toolu** bakes that discipline back in — as hooks that gate every edit, skills that enforce a design → review → build → review → test cadence, and a plugin registry so language-specific rules ride along automatically. Runs on **Claude Code and Codex**.

[![Release](https://img.shields.io/github/v/release/Falconiere/toolu?sort=semver&color=d97757)](https://github.com/Falconiere/toolu/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Tests](https://img.shields.io/badge/tests-1700%2B%20passing-brightgreen)](#testing)
[![Hosts](https://img.shields.io/badge/hosts-Claude%20Code%20%7C%20Codex-d97757)](#install)
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

The headline feature. When `rust-quality`, `ts-quality`, and/or `python-quality` is installed, every Rust/TypeScript/Python file the agent edits is checked on the spot. Limits are **config-driven** (project/user override → the active native linter's `max-lines` → built-in default), and the gate is **multi-slot**: a failing test command and a failing file check are tracked independently, so fixing one never silently masks the other.

<table>
<tr><th align="left">TypeScript</th><th align="left">Rust</th><th align="left">Python</th></tr>
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

</td><td>

- File / function line limits
- No bare `except:` / one-line `except: pass`
- No blanket `# noqa` / `# type: ignore`
- No mocks in `test_*.py` / `*_test.py`
- Tests colocated as `test_*.py` next to the module
- Docstrings on public `def`/`class` (advisory)

</td></tr>
</table>

The rule isn't "warn and move on" — it's a hard gate: **no new task while the gate is red.** Found a real problem? Fix it in code. (There's no "disable this check" escape hatch by design.)

## Install

toolu has first-class packages for Claude Code and Codex. Codex support covers
the CLI, IDE extension, and ChatGPT desktop Codex on macOS and Linux.

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
/plugin install python-quality@toolu # Python quality gates
/plugin install ast-grep@toolu       # structural code search & rewrite
/plugin install comemory@toolu       # persistent cross-session memory
/plugin install context7@toolu       # live library documentation lookup
/plugin install exa-search@toolu     # web / code / URL search + research
```

> **Note** — `comemory`, `rust-quality`, `ts-quality`, and `python-quality` depend on `toolu`; `ast-grep`, `context7`, and `exa-search` are standalone (zero deps). The only external-binary dependency in the bundle is `comemory` (see below). `caveman` and `code-simplifier` are **optional, recommended companions**, not required — install them only if you want caveman mode or the pre-simplify pass; when absent, `toolu` falls back (the `push-review` gate uses the built-in `/code-review`, and `code-simplifier` is invoked only if installed). Adding the marketplaces in step 1 lets Claude Code resolve those companions automatically. The `push-review` gate is **reviewer-agnostic** — it does not force you to use caveman: `caveman:cavecrew-reviewer` is preferred when present, otherwise the built-in `/code-review` skill satisfies the gate.

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

### Codex

Add the marketplace and install the core first:

```bash
codex plugin marketplace add Falconiere/toolu
codex plugin add toolu@toolu
```

Then install whichever domain plugins you want:

```bash
codex plugin add rust-quality@toolu
codex plugin add ts-quality@toolu
codex plugin add python-quality@toolu
codex plugin add ast-grep@toolu
codex plugin add comemory@toolu
codex plugin add context7@toolu
codex plugin add exa-search@toolu
codex plugin add git-better@toolu
codex plugin add jira@toolu
codex plugin add toolu-review@toolu
codex plugin add pr-babysit@toolu
codex plugin add statusline@toolu
codex plugin add agent-browser@toolu
```

Plugins that depend on the core detect a missing installation at SessionStart
and print the exact repair command: `codex plugin add toolu@toolu`.

Codex discovers the same canonical workflows as namespaced skills:

- `$toolu:commit`, `$toolu:review-and-commit`
- `$comemory:setup`
- `$statusline:status`
- `$pr-babysit:babysit`
- `$toolu:setup` to preview, install, update, back up, or remove the five bundled Codex agent profiles

Codex requires explicit hash-based trust before plugin hooks execute. Review and
trust the installed hooks through Codex's `/hooks` interface after installation;
a changed hook must be trusted again. Codex cloud and Windows are not supported
in this release. Custom agent profiles are installed locally under
`${CODEX_HOME:-~/.codex}/agents`, so they are also unavailable in Codex cloud.

## What's inside

Fourteen plugins, one marketplace. Every plugin ships synchronized Claude and
Codex manifests at the repository version (`4.10.0` here). Install the core
alone, or add the domain plugins.

| Group | Plugin | Version | What it does |
|--------|--------|:-------:|--------------|
| Core | **`toolu`** | `4.10.0` | Registry-driven hook engine, 8-phase workflow, commit workflows, model routing, push-review gate, and custom-agent templates. |
| Quality gate | **`rust-quality`** | `4.10.0` | Rust post-edit checks — size limits, `.unwrap()`/`.expect()` bans, no `unsafe`, no lint suppression, flat real-data tests. |
| Quality gate | **`ts-quality`** | `4.10.0` | TypeScript post-edit checks — size limits, imports, type assertions/guards, duplicate types, and colocated real-data tests. |
| Quality gate | **`python-quality`** | `4.10.0` | Python post-edit checks — size limits, no suppression (bare `except:`/`# noqa`/`# type: ignore`), docstrings, colocated real-data tests. |
| Code intel | **`ast-grep`** | `4.10.0` | Structural code search and rewrite plus a registry-driven text-to-AST nudge. |
| Code intel | **`comemory`** | `4.10.0` | Persistent memory and code indexing with host-native setup, scope enforcement, and status publishing. |
| Browser | **`agent-browser`** | `4.10.0` | Token-lean browser automation through accessibility-tree snapshots and stable element references. |
| Knowledge | **`context7`** | `4.10.0` | Live library documentation and code examples through Context7. |
| Knowledge | **`exa-search`** | `4.10.0` | Web, code, URL search, and deep research through Exa. |
| Workflow | **`git-better`** | `4.10.0` | Lean git reads and cached repository-convention discovery. |
| Workflow | **`jira`** | `4.10.0` | Jira Cloud and Server/DC search plus safe issue workflow operations. |
| Workflow | **`toolu-review`** | `4.10.0` | Pre-push review matching the CI review bot and writing review attestations. |
| Workflow | **`pr-babysit`** | `4.10.0` | Strict PR clearance through Claude cron or a durable Codex goal with isolated worktrees. |
| Status | **`statusline`** | `4.10.0` | Persistent Claude statusline plus an explicit Codex repository/gate status report. |

Beyond the plugins, the core (`toolu`) also ships:

- **\`push-review\` gate** — blocks \`git push\` on a feature branch until the diff has been run through an accepted reviewer (\`caveman:cavecrew-reviewer\` when installed, the built-in \`/code-review xhigh --fix\` skill, or the \`toolu-review:review\` skill), with a round cap (5 rewrites against an unchanged diff) that escalates instead of looping forever. The state file lives under the pushed repo's own root, so `git -C <worktree> push` is gated on the worktree's branch and diff.
- **docs-sync backstop** — on `git push`, an **advisory** (never a block) when the branch diff changes code but no documentation surface (README, `docs/` guides, `SKILL.md` triggers) — a nudge to keep user-facing docs in sync with behavior. Silenced by a diff-`sha`-keyed attestation; surfaces are tunable via `docsSync.*` ([config](docs/config.md#docs-sync-surfaces-docssync)). Pairs with the "Docs in sync" convention the workflow skills enforce.
- **Commit workflows** — Claude exposes `/commit` and `/review-and-commit`; Codex exposes `$toolu:commit` and `$toolu:review-and-commit`. Both read the same canonical workflow files, preventing host drift.
- **Model routing** — delegated work is tiered by its *class*, not its phrasing. Claude defaults to Haiku/Sonnet/Opus aliases; Codex defaults to Luna/medium for mechanical work, Terra/medium for exploration and implementation, Terra/high for review, and Sol/high for synthesis and architecture. Both mappings are configurable in [config](docs/config.md#model-routing-models).
- **Tier-pinned agents** — Claude reads the bundled agent definitions directly. `$toolu:setup` manages Codex TOML profiles for `quick-task` (Luna/medium, read-only), `deep-explore` and `research-agent` (Terra/medium, read-only), `implementer` (Terra/medium, workspace-write), and `architect` (Sol/high, read-only), with previews, conflict refusal, timestamped backups, and recoverable removal.
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

- **`brainstorm`** surfaces intent, constraints, and prior art before any code — it sweeps the design dimensions, picks a default for each, and proceeds.
- **`spec`** writes a design contract to `docs/toolu/specs/`; **`spec-review`** audits it.
- **`plan`** turns the spec into concrete steps; **`plan-review`** checks it's executable.
- **`execution`** drives the plan with verification checkpoints; **`execution-review`** is hard-focused on error handling.
- **`test`** enforces real-data tests (no mocks), colocated by language.

Mechanical work (renames, dep bumps, one-liners) skips the ceremony — each skill declares when *not* to fire.

The workflow skills, plus `ast-grep`, `agent-memory` (from `comemory`), `context7`, and `exa-search`, all run off the same shell hook engine. The standalone `deep-research` skill combines `exa-search` and `context7` fan-out into cited reports under `docs/research/`.

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
    RQ -- "assemble concern fragments at SessionStart" --> R[("registry<br/>host config dir/toolu/")]
    TQ -- "one assembled module per language" --> R
    AG -- "namespaced plugin__name.sh" --> R
    CM -- "namespaced plugin__name.sh" --> R
    R --> D
    D -- "runs a module only while its plugin is installed" --> OUT([enforced edit])
```

At `SessionStart`, each domain plugin's `register.sh` contributes to the registry as `<plugin-spec>__<name>.sh` — `ast-grep` and `comemory` mirror their `hooks/<event>.d/*.sh` one-to-one, while `rust-quality`/`ts-quality`/`python-quality` assemble their ordered `hooks/concerns/` fragments into a single module per language. The core executes those copies **only while the owning plugin is installed** — uninstall the plugin and its rules vanish, fail-closed.

<details>
<summary><b>Full repository layout</b></summary>

```text
.
├── docs/                       # Runtime config schema, design notes
└── plugins/
    ├── toolu/                  # Core plugin: hook engine + process gates
    │   ├── .claude-plugin/     # Claude Code plugin.json manifest
    │   ├── .codex-plugin/      # Codex plugin.json manifest
    │   ├── skills/             # brainstorm, spec(+review), plan(+review),
    │   │                       #   execution(+review), test, deep-research
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
    ├── python-quality/         # Python PostToolUse quality fragments, assembled at SessionStart
    ├── statusline/             # optional gate-aware statusline + SessionStart symlink hook
    ├── pr-babysit/             # Claude command + Codex skill + strict shared workflow
    └── toolu-review/            # toolu-review:review skill + push-review state writer
```

</details>

## Configuration

Toggle individual skills, hooks, or MCP servers without uninstalling anything.
Claude uses `~/.claude/toolu.config.json` and `<repo>/.claude/toolu.config.json`;
Codex uses `${CODEX_HOME:-~/.codex}/toolu.config.json` and
`<repo>/.codex/toolu.config.json`. `TOOLU_CONFIG_DIR` and
`TOOLU_PROJECT_CONFIG_DIRNAME` override both roots. Defaults are opt-out — no
file required.

```json
{
  "version": 1,
  "skills": { "comemory": false }
}
```

Quality-gate thresholds (file/function/impl line limits) are configurable per project and per language. Full schema and examples: [`docs/config.md`](./docs/config.md).

## Testing

The hook engine and language gates are covered by **1700+ [bats](https://github.com/bats-core/bats-core) tests**, all run in CI on every push, including real temporary-home Codex install/remove smoke coverage:

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
- Review the Release PR, then **merge it to cut the release** — the only manual step. It bumps `package.json` and every Claude and Codex plugin manifest to the new version, updates `CHANGELOG.md`, tags `vX.Y.Z`, and publishes the GitHub Release. The marketplace re-extracts a plugin when its manifest version changes.

`tooling/release.sh` is **deprecated**, kept only as a manual escape hatch for when the automation is unavailable.

## References

- [Claude Code docs](https://docs.claude.com/en/docs/claude-code) ·
  [Skills](https://docs.claude.com/en/docs/claude-code/skills) ·
  [Subagents](https://docs.claude.com/en/docs/claude-code/sub-agents) ·
  [Slash commands](https://docs.claude.com/en/docs/claude-code/slash-commands) ·
  [Hooks](https://docs.claude.com/en/docs/claude-code/hooks) ·
  [Plugins](https://docs.claude.com/en/docs/claude-code/plugins)
- [Codex plugins](https://developers.openai.com/plugins/build/plugins) ·
  [Hooks and trust](https://learn.chatgpt.com/docs/hooks) ·
  [Custom subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents)

## License

[MIT](./LICENSE) © [Falconiere Barbosa](https://github.com/falconiere)
