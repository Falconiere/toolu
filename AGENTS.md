# AGENTS.md

## Overview

**toolu** — Engineering discipline for AI coding agents. A plugin marketplace that bakes code-quality rules into every edit via hooks, skills, and a runtime registry. Runs on Claude Code.

## Tech Stack

- **Shell** (bash) — all hooks, gate logic, registry. The canonical language. `set -euo pipefail`, shellcheck-clean.
- **bats** — test framework. ~1340 tests, colocated in `__tests__/` dirs. Run via `bats -r plugins tooling`.
- **Bun** — package manager (see `bun.lock`). Also runs the bats suite via the `test`/`test:shell` scripts.
- **shellcheck** — `bun run lint:shell` (`tooling/shellcheck.sh`) lints standalone scripts directly and each `hooks/concerns/` dir as its ASSEMBLED module, since fragments are partials of one script.

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

- **One version for the whole repo** — `package.json` and **every** plugin's `plugin.json` share the same `vX.Y.Z`, matching the git tag. They are bumped together on each release.
- The auto-update gate re-extracts a plugin **only** when its `plugin.json` version changes; because all plugins move in lockstep, every release re-extracts every plugin (accepted — keeps the marketplace simple and avoids stale code).

## Release Process

Releases are automated with **release-please** (`.github/workflows/release-please.yml`; config in `release-please-config.json` + `.release-please-manifest.json`). No manual version bumps or tags.

### 1. Merge Conventional Commits to `main`

The repo is a **single release-please package** (`"."`, `release-type: node`), so **any** commit feeds the release regardless of path — a `feat` in `tooling/` or `.github/` counts the same as one under `plugins/`. `feat` / `fix` / `feat!` (or `BREAKING CHANGE`) drive minor / patch / major bumps; `chore` / `docs` / `ci` / `refactor` bump nothing.

### 2. Review and merge the Release PR

release-please maintains a single batched **Release PR** that bumps `package.json` `version` (owned natively by the `node` strategy) plus every plugin's `plugin.json` `version` (via `extra-files`), and updates `CHANGELOG.md`. Edit the PR body to curate notes if you like, then **merge it to cut the release** — the only manual step.

### 3. Tags + GitHub Releases (automatic)

On merge, release-please tags the release `vX.Y.Z` (`component: toolu` + `include-component-in-tag: false` keeps the historical component-less tag shape) and publishes the GitHub Release with the generated changelog as the body. The auto-update gate then re-extracts every plugin (all `plugin.json` versions changed).

### Notes

- **Baseline:** `.release-please-manifest.json` pins the last released version (`"."` = the current `vX.Y.Z`); release-please collects commits since that tag. No `bootstrap-sha` needed — the existing `vX.Y.Z` tag is the baseline.
- **`tooling/release.sh` is deprecated** — kept only as a manual escape hatch for when the automation is unavailable; it is no longer part of the normal flow.

## CI Pipeline

All workflows live in `.github/workflows/`:

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `tests.yml` | push/PR to `main` | Typecheck (bun), bats suite, colocated-test layout enforcement, deterministic benchmarks, context-budget guard |
| `release-please.yml` | push to `main` | Maintains the batched Release PR; on merge, bumps `package.json` + every `plugin.json` to the new `vX.Y.Z`, updates the changelog, tags, and publishes the GitHub Release |
| `toolu-review.yml` | PR opened/synchronize | CI review bot |

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
| `plugins/toolu/hooks/lib/detect.sh` | Code-line counter, comemory version detection, tool availability, `git push` detection (`is_git_push`) + push target repo resolution (`push_target_root`) |
| `plugins/*/hooks/register.sh` | SessionStart: syncs hook modules into the toolu runtime registry under the agent config dir |
| `plugins/*/hooks/hooks.json` | Claude Code hook routing (event → script path + matcher) |
| `tooling/shellcheck.sh` | shellcheck gate: standalone scripts + each concerns dir assembled into the module that actually runs |
| `docs/config.md` | Full config schema reference (`skills`, `hooks`, `mcp`, `lang` thresholds, `docsSync`) |
| `tooling/release.sh` | **Deprecated** manual escape hatch — atomic version bump for the monorepo (`--dry-run`, `--no-notes`); releases are normally automated by release-please |
| `tooling/templates/release-notes.md` | Skeleton the release script copies + fills for the per-release notes |
| `plugins/toolu/scripts/context-budget.sh` | CI-only: caps injected-context footprint |

## Contributing

1. **Pick the right home** — skill vs. agent vs. command vs. hook — use existing siblings as templates.
2. **Add tests** — colocated `__tests__/*.bats` for any hook logic. No mocks.
3. **Verify in a real session** before committing.
4. **Conventional Commits**: `feat(scope):`, `fix(scope):`, etc.
5. **Run every CI check locally**: `bun run test` runs `lint:shell` → `test:shell` in sequence.

## Common Tasks

- **Add a skill**: `plugins/<name>/skills/<skill>/SKILL.md` + optional `scripts/` and `references/`.
- **Add a hook concern**: `plugins/<quality>/hooks/concerns/NN-concern.sh` + `__tests__/concern.bats`. The NN prefix controls assembly order.
- **Add a standalone hook module**: `plugins/<plugin>/hooks/<event>.d/module.sh` + `register.sh` to wire it into the registry.
- **Add a new plugin**: create `plugins/<name>/.claude-plugin/plugin.json` + `README.md` (from `tooling/templates/plugin-README.md`). Add tests.
- **Run subset of tests**: `bats plugins/<plugin>/hooks/__tests__/`.

## Version

Current: `3.3.0` (see `package.json` and `plugins/toolu/.claude-plugin/plugin.json`). License: MIT.
