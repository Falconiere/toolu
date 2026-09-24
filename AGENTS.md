# AGENTS.md

## Overview

**toolu** — plugin marketplace that enforces code-quality rules through hooks, skills, and a runtime registry. Runs on Claude Code.

## Agent instructions

This file is the source of truth. Codex, Cursor, and Claude Code read it directly. Put instructions here. It is a docs-sync surface, so a stale copy fails the gate.

## Tech stack

- **bash** — hooks, gates, registry. `set -euo pipefail`, shellcheck-clean.
- **bats** — colocated `__tests__/`. `bash tooling/bats-run.sh` runs `plugins`, `benchmarks`, `tooling`, `packages`, and `tools` (files in parallel, tests in a file serial). `bun run test:shell:serial` is the serial path.
- **Bun** — `bun.lock`. `bun run test` is `lint:shell`, then `test:context-budget`, then `test:shell`.
- **`toolu` CLI** — `tools/toolu-cli`, a Node bundle published to npm as `@toolu/cli`. Installs plugins across hosts by shelling out to each host's own plugin CLI. See `docs/cli.md`.
- **shellcheck** — `bun run lint:shell` lints standalone scripts and each `hooks/concerns/` directory as the assembled module.

## Plugin layout

Self-contained under `plugins/<name>/`. No symlinks out.

```
plugins/<name>/
  .claude-plugin/plugin.json   # name, version, description, dependencies
  README.md                    # tooling/templates/plugin-README.md
  hooks/hooks.json             # Claude Code event routing
  hooks/register.sh            # SessionStart: sync modules into the registry
  hooks/concerns/              # NN-description.sh fragments, assembled at SessionStart
  hooks/<event>.d/             # standalone (pre-tools.d, post-tools.d, session-start.d)
  hooks/__tests__/             # colocated bats
  skills/<skill>/SKILL.md
  commands/<name>.md
  agents/<name>.md
  scripts/
  settings/                    # core only
```

Root `package.json`, Bun workspace packages (`packages/toolu-core`, `tools/toolu-opencode`, `tools/toolu-conformance`, `tools/toolu-cli`), and every `plugin.json` share one `vX.Y.Z`, matching the git tag. A plugin is re-extracted only when its `plugin.json` version changes, so a release re-extracts all of them.

## Releases

release-please (`.github/workflows/release-please.yml`). No manual bumps or tags.

Any Conventional Commit on `main` counts, any path. `feat` / `fix` / `feat!` bump minor / patch / major. `chore` / `docs` / `ci` / `refactor` bump nothing. Merge the Release PR to publish: it bumps root `package.json`, Bun workspace packages under `packages/` and `tools/`, and every `plugin.json`, updates `CHANGELOG.md`, tags `vX.Y.Z` with no component prefix, and opens the GitHub Release. `tooling/release.sh` is a deprecated escape hatch. OpenCode install: `docs/opencode.md`.

**npm.** `release-please.yml` calls `npm-publish.yml` after the Release, which publishes `@toolu/core`, `@toolu/opencode`, then `@toolu/cli` with provenance via OIDC and `secrets.NPM_TOKEN`, and does not go green until each one resolves on the registry (a first publish can take minutes to stop answering 404). `@toolu/conformance` stays `private`; it is an internal harness. The tarball's file list is gated by `bun run test:pack`.

## CI

| Workflow | When | What |
|----------|------|------|
| `tests.yml` | push/PR to `main`, or a manual run. Skipped when the diff is only release version files and `CHANGELOG.md` | shellcheck, bats, colocated-test layout, deterministic benchmarks, context budget |
| `release-please.yml` | push to `main` | Release PR; on merge, tag and GitHub Release |
| `toolu-review.yml` | PR opened/synchronize, except a release-version-and-changelog-only diff | `falconiere/toolu-ghactions/code-review@v8` (Jev on: `JEV_ENABLED` + `JEV_MODEL_ID: typesafe/jev-1.13`) |

A `.bats` file outside `__tests__/` fails CI. Benchmarks are hermetic. Context budget caps the Session Protocol, per-language docs, and skill descriptions.

## Key files

| File | Purpose |
|------|---------|
| `plugins/toolu/hooks/pre-tools/mod.sh` | Pre-tool dispatcher: protected files, bash, MCP, commit, quality |
| `plugins/toolu/hooks/post-tools/mod.sh` | Quality checks on edited files |
| `plugins/toolu/hooks/lib/quality-config.sh` | Thresholds: override, then linter config, then default |
| `plugins/toolu/hooks/lib/detect.sh` | Line counts, tool availability, `is_git_push`, `push_target_root`, `push_target_branch` |
| `plugins/pr-babysit/scripts/babysit-tick.sh` | Babysit tick. Writes go through `reply-thread.sh`, `resolve-thread.sh`, `record.sh` |
| `plugins/*/hooks/register.sh` | SessionStart registry sync |
| `plugins/*/hooks/hooks.json` | Claude Code hook routing |
| `tools/toolu-cli/src/cli.ts` | CLI entry (`npx @toolu/cli@latest …`, or `toolu …` once installed): parses argv, resolves the host, dispatches a noun |
| `tools/toolu-cli/src/plugins/install.ts` | Dependency-ordered install; core failure stops dependents, others continue |
| `tools/toolu-cli/src/host/` | Per-host adapters normalizing `plugin list --json` into one shape |
| `tooling/src/pack-inventory.ts` | Published-tarball file-list gate |
| `tooling/shellcheck.sh` | shellcheck gate |
| `docs/config.md` | Config schema |
| `plugins/toolu/scripts/context-budget.sh` | Injected-context word ceilings |

## Contributing

1. Match an existing skill, agent, command, or hook.
2. Colocate `__tests__/*.bats`. No mocks.
3. Verify in a real session, then commit with `feat(scope):` or `fix(scope):`.
4. `bun run test` before pushing.

- Skill: `plugins/<name>/skills/<skill>/SKILL.md`
- Concern: `plugins/<quality>/hooks/concerns/NN-concern.sh` plus a bats test. `NN` is assembly order.
- Hook module: `plugins/<plugin>/hooks/<event>.d/module.sh` and `register.sh`
- Plugin: `plugins/<name>/.claude-plugin/plugin.json` and a README from `tooling/templates/plugin-README.md`
- Subset: `bats plugins/<plugin>/hooks/__tests__/`

Version is `package.json` and every `plugin.json`. License: MIT.
