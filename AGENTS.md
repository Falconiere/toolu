# AGENTS.md

## Overview

**toolu** — Engineering discipline for AI coding agents. A plugin marketplace that bakes code-quality rules into every edit via hooks, skills, and a runtime registry. Runs on Claude Code, pi, and opencode.

## Tech Stack

- **Shell** (bash) — all hooks, gate logic, registry. The canonical language. `set -euo pipefail`, shellcheck-clean.
- **TypeScript** — pi extension (`pi/extensions/toolu.ts`) and opencode extension (`opencode/extensions/toolu.ts`), typecheck only. Both share the same shell hook engine — they're per-runtime adapters.
- **bats** — test framework. ~656 tests, colocated in `__tests__/` dirs. Run via `bats -r plugins tooling`.
- **Bun** — package manager (see `bun.lock`). `bun run typecheck` for TS, `bun run test:ts` for the adapter unit tests.

## Plugin Anatomy

Every plugin lives under `plugins/<name>/` and is **self-contained** — no symlinks out, no content outside its root. A plugin looks like this:

```
plugins/<name>/
  .claude-plugin/plugin.json   # manifest: name, version, description, dependencies, skills/agents/commands/hooks
  README.md                    # from tooling/templates/plugin-README.md
  hooks/                       # (optional) hook modules + tests
    hooks.json                 #   route patterns → hook scripts (Claude Code)
    register.sh                #   SessionStart: sync hook modules into the toolu runtime registry
    concerns/                  #   quality-gate: ordered fragments (NN-description.sh), assembled at SessionStart
    <event>.d/                 #   standalone: match event name (pre-tools.d, post-tools.d, session-start.d)
    __tests__/                 #   colocated bats tests
  skills/<skill>/SKILL.md      # skill instructions + optional scripts/, references/
  commands/<name>.md           # slash command or prompt template
  agents/<name>.md             # sub-agent definition
  scripts/                     # helper scripts called by skills/hooks
  settings/                    # (core only) reusable settings fragments (denylists, allowlists, example config)
```

### Plugin versioning

- **`toolu` anchors the monorepo** — its `plugin.json` version and `package.json` version always match the git tag.
- Every **other** plugin carries its **own independent** semver in its `plugin.json`.
- The auto-update gate re-extracts a plugin **only** when its `plugin.json` version changes — so a stale version silently ships stale code.

## Release Process

Releases are automated with **release-please** (`.github/workflows/release-please.yml`; config in `release-please-config.json` + `.release-please-manifest.json`). No manual version bumps or tags.

### 1. Merge Conventional Commits to `main`

Scope each commit with the plugin it changes (`feat(jira): …`, `fix(toolu): …`). release-please routes a commit to a plugin **component** by the files it touches under `plugins/<name>/` (path match, not the scope string — but they align, since each plugin is its own dir). `feat` / `fix` / `feat!` (or `BREAKING CHANGE`) drive minor / patch / major bumps. `chore` / `docs` / `ci` / `refactor`, and commits touching only repo-root paths (`tooling/`, `.github/`, `docs/`), bump nothing.

### 2. Review and merge the Release PR

release-please maintains a single batched **Release PR** (`separate-pull-requests: false`) that bumps each affected plugin's `plugin.json` `version` and updates its `CHANGELOG.md`. Edit the PR body to curate notes if you like, then **merge it to cut the release** — the only manual step.

### 3. Tags + GitHub Releases (automatic)

On merge, release-please tags every bumped plugin (`toolu` → `vX.Y.Z` via `include-component-in-tag: false`; others → `<plugin>-vX.Y.Z`) and publishes the GitHub Release(s) with the generated changelog as the body. The auto-update gate re-extracts a plugin when its `plugin.json` version changes. A non-fatal `[skip ci]` workflow step then syncs root `package.json` to the toolu version (release-please cannot update a file above a component dir).

### Notes

- **First-run baseline:** `bootstrap-sha` (top of `release-please-config.json`) pins adoption HEAD so the 12 currently untagged plugins do not all release on the first run; `toolu` baselines off its existing `vX.Y.Z` tag.
- **`tooling/release.sh` is deprecated** — kept only as a manual escape hatch for when the automation is unavailable; it is no longer part of the normal flow.

## CI Pipeline

All workflows live in `.github/workflows/`:

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `tests.yml` | push/PR to `main` | Typecheck (bun), bats suite, colocated-test layout enforcement, deterministic benchmarks, context-budget guard |
| `release-please.yml` | push to `main` | Maintains the batched Release PR; on merge, bumps plugin versions + changelogs, tags, publishes GitHub Release(s), and syncs root `package.json` |
| `code-review.yml` | PR opened/synchronize | CI review bot |
| `codeql.yml` | cron + push to `main` | CodeQL analysis |
| `secret-scan.yml` | push/PR | Secret scanning |
| `claude-mention.yml` | issue/PR comment | Claude Code mention automation |

### Tests CI details

- **Colocated test enforcement**: any `.bats` file outside a `__tests__/` dir fails the build.
- **Deterministic benchmarks**: hermetic, no API key; compares full-file read bytes vs ast-grep targeted-match bytes (offline token heuristic).
- **Context budget guard**: word ceilings on the Session Protocol + per-language docs and every skill `description`. Fails the build if any surface exceeds budget, preventing silent baseline regrowth.

## Key Files

| File | Purpose |
|------|---------|
| `plugins/toolu/hooks/pre-tools/mod.sh` | Pre-tool gate dispatcher (protected files, bash commands, MCP blocker, commit gate, quality gate) |
| `plugins/toolu/hooks/post-tools/mod.sh` | Post-tool gate dispatcher (quality checks on edited files) |
| `plugins/toolu/hooks/lib/quality-config.sh` | Quality threshold resolver (override → linter config → built-in default) |
| `plugins/toolu/hooks/lib/detect.sh` | Code-line counter, comemory version detection, tool availability |
| `plugins/*/hooks/register.sh` | SessionStart: syncs hook modules into the toolu runtime registry under the agent config dir |
| `plugins/*/hooks/hooks.json` | Claude Code hook routing (event → script path + matcher) |
| `pi/extensions/toolu.ts` | pi extension — runs the same shell hooks as Claude Code, surfaces gate status in pi's footer |
| `opencode/extensions/toolu.ts` | opencode plugin — same shell hooks, opencode's `tool.execute.before` / `tool.execute.after` / `experimental.session.compacting` event surface |
| `opencode/agents/*.md`, `opencode/commands/*.md` | opencode-format agents and slash commands (frontmatter translated from the `plugins/*/agents` and `plugins/*/commands` source of truth) |
| `tooling/install-opencode.sh` | Project/global installer: copies agents, commands, and the adapter into `.opencode/`, wires `skills.paths` in `opencode.json`, runs every plugin's `register.sh` |
| `docs/config.md` | Full config schema reference (`skills`, `hooks`, `mcp`, `lang` thresholds, `docsSync`) |
| `docs/opencode.md` | opencode adapter architecture, install, smoke test, troubleshooting |
| `tooling/release.sh` | **Deprecated** manual escape hatch — atomic version bump for the monorepo (`--dry-run`, `--no-notes`); releases are normally automated by release-please |
| `tooling/templates/release-notes.md` | Skeleton the release script copies + fills for the per-release notes |
| `plugins/toolu/scripts/context-budget.sh` | CI-only: caps injected-context footprint |

## Contributing

1. **Pick the right home** — skill vs. agent vs. command vs. hook — use existing siblings as templates.
2. **Add tests** — colocated `__tests__/*.bats` for any hook logic. No mocks.
3. **Verify in a real session** before committing.
4. **Conventional Commits**: `feat(scope):`, `fix(scope):`, etc.
5. **Run full CI locally**: `bun run test` (runs `test:shell` then `test:ts` then `typecheck`).

## Common Tasks

- **Add a skill**: `plugins/<name>/skills/<skill>/SKILL.md` + optional `scripts/` and `references/`. Skills auto-discover on opencode via `skills.paths` (set by `tooling/install-opencode.sh`).
- **Add a hook concern**: `plugins/<quality>/hooks/concerns/NN-concern.sh` + `__tests__/concern.bats`. The NN prefix controls assembly order.
- **Add a standalone hook module**: `plugins/<plugin>/hooks/<event>.d/module.sh` + `register.sh` to wire it into the registry.
- **Add a new plugin**: create `plugins/<name>/.claude-plugin/plugin.json` + `README.md` (from `tooling/templates/plugin-README.md`). Add tests. If the plugin ships skills/agents, mirror them in `opencode/agents/` and `opencode/commands/` for opencode discovery.
- **Add an opencode agent/command**: drop the opencode-format file in `opencode/agents/<name>.md` or `opencode/commands/<name>.md` (frontmatter uses `permission:` not `tools:`, `provider/model-id` not `model: sonnet`).
- **Install on opencode**: `bash tooling/install-opencode.sh` (project install) or `bash tooling/install-opencode.sh --global` (user install). Re-runnable and idempotent.
- **Run subset of tests**: `bats plugins/<plugin>/hooks/__tests__/`.
- **Typecheck**: `bun run typecheck`.

## Version

Current: `1.19.0` (see `package.json` and `plugins/toolu/.claude-plugin/plugin.json`). License: MIT.
