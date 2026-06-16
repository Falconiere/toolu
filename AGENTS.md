# AGENTS.md

## Overview

**toolu** — Engineering discipline for AI coding agents. A plugin marketplace that bakes code-quality rules into every edit via hooks, skills, and a runtime registry. Runs on Claude Code and pi.

## Tech Stack

- **Shell** (bash) — all hooks, gate logic, registry. The canonical language. `set -euo pipefail`, shellcheck-clean.
- **TypeScript** — pi extension (`pi/extensions/toolu.ts`), typecheck only.
- **bats** — test framework. ~638 tests, colocated in `__tests__/` dirs. Run via `bats -r plugins`.
- **Bun** — package manager (see `bun.lock`). `bun run typecheck` for TS.

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

### 1. Cut a release

```sh
tooling/release.sh <major.minor.patch>
```

This is an **atomic, two-pass** script:

| Pass | What happens |
|------|-------------|
| 1 (validate) | Resolves every version bump without writing anything. If a single manifest is malformed, the whole release aborts — no partial writes. |
| 2 (apply) | Edits `"version"` fields **in place by line** (sed, not jq, so key ordering is preserved). |

Version bump rules:

| Target | Rule |
|--------|------|
| `package.json` | Set to `<new-version>` |
| `plugins/toolu/.claude-plugin/plugin.json` | Set to `<new-version>` (anchors the monorepo) |
| Every **other** plugin whose files changed since the last `v*.*.*` tag | **PATCH bump** of its own independent version |
| Unchanged plugins (no diff since last tag) | **Left alone** |

The script only accepts strict `major.minor.patch` (no pre-release/build/extra segments). A malformed manifest version in any changed plugin **aborts without touching any file**.

### 2. Tag

```sh
git tag -a v<version> -m "v<version>"
git push --follow-tags
```

### 3. CI gate: tag must match manifest

CI's `release.yml` **verifies** that `git tag (vX.Y.Z)` matches `plugins/toolu/.claude-plugin/plugin.json` version. If they diverge, the release is blocked. This is the safety net for the "stale version" problem.

### 4. Write release notes

Create `docs/releases/v<version>.md` with:

```markdown
# toolu vX.Y.Z

Released: YYYY-MM-DD

## Highlights

<2-3 sentence summary>

## Included changes since v<prev>

- `<plugin>`: <change description>. (`<plugin>` <old-ver> → <new-ver>)
- …
```

### 5. GitHub Release

On tag push, `release.yml` runs `gh release create` with `--generate-notes`. The tag gate (step 3) must pass first.

## CI Pipeline

All workflows live in `.github/workflows/`:

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `tests.yml` | push/PR to `main` | Typecheck (bun), bats suite, colocated-test layout enforcement, deterministic benchmarks, context-budget guard |
| `release.yml` | tag `v*.*.*` | Tag→manifest version check, `gh release create` |
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
| `docs/config.md` | Full config schema reference (`skills`, `hooks`, `mcp`, `lang` thresholds, `docsSync`) |
| `tooling/release.sh` | Atomic version bump for the monorepo |
| `plugins/toolu/scripts/context-budget.sh` | CI-only: caps injected-context footprint |

## Contributing

1. **Pick the right home** — skill vs. agent vs. command vs. hook — use existing siblings as templates.
2. **Add tests** — colocated `__tests__/*.bats` for any hook logic. No mocks.
3. **Verify in a real session** before committing.
4. **Conventional Commits**: `feat(scope):`, `fix(scope):`, etc.
5. **Run full CI locally**: `bats -r plugins tooling && bun run typecheck`.

## Common Tasks

- **Add a skill**: `plugins/<name>/skills/<skill>/SKILL.md` + optional `scripts/` and `references/`.
- **Add a hook concern**: `plugins/<quality>/hooks/concerns/NN-concern.sh` + `__tests__/concern.bats`. The NN prefix controls assembly order.
- **Add a standalone hook module**: `plugins/<plugin>/hooks/<event>.d/module.sh` + `register.sh` to wire it into the registry.
- **Add a new plugin**: create `plugins/<name>/.claude-plugin/plugin.json` + `README.md` (from `tooling/templates/plugin-README.md`). Add tests. Wire into `pi/package.json` if it ships a skill.
- **Run subset of tests**: `bats plugins/<plugin>/hooks/__tests__/`.
- **Typecheck**: `bun run typecheck`.

## Version

Current: `1.19.0` (see `package.json` and `plugins/toolu/.claude-plugin/plugin.json`). License: MIT.
