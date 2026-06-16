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

### 1. Cut a release

```sh
# Always preview first:
tooling/release.sh --dry-run <major.minor.patch>

# Then apply (also auto-drafts docs/releases/v<version>.md):
tooling/release.sh <major.minor.patch>
```

This is an **atomic, two-pass** script:

| Pass | What happens |
|------|-------------|
| 1 (validate) | Resolves every version bump without writing anything. If a single manifest is malformed, the whole release aborts — no partial writes. |
| 2 (apply) | Edits `"version"` fields **in place by line** (sed, not jq, so key ordering is preserved). |
| 3 (notes) | Auto-drafts `docs/releases/v<version>.md` as a template skeleton with per-plugin `(old → new)` attribution. Refuses to overwrite an existing file (pass `--no-notes` to skip). |

`--dry-run` / `--plan` prints the full bump plan — anchor versions, per-plugin `old → new` transitions, per-plugin `git diff --shortstat` against the last `v*.*.*` tag, and the list of plugins that will be **skipped** (no diff since last tag) — then writes nothing. Re-run without `--dry-run` to apply.

Version bump rules:

| Target | Rule |
|--------|------|
| `package.json` | Set to `<new-version>` |
| `plugins/toolu/.claude-plugin/plugin.json` | Set to `<new-version>` (anchors the monorepo) |
| Every **other** plugin whose files changed since the last `v*.*.*` tag | **PATCH bump** of its own independent version |
| Unchanged plugins (no diff since last tag) | **Left alone** |

The script only accepts strict `major.minor.patch` (no pre-release/build/extra segments). A malformed manifest version in any changed plugin **aborts without touching any file**.

### 2. Edit the release notes

The apply step drafts `docs/releases/v<version>.md` (template at `tooling/templates/release-notes.md`) with `<!-- TODO ... -->` placeholders for `## Highlights`, `## Upgrade notes`, and each per-plugin bullet. Fill in the placeholders, delete the comment markers, and **delete the entire `## Upgrade notes` section** (header + TODO) if there are no user-facing steps. The shape matches the existing v1.10.0 / v1.12.0 notes:

```markdown
# toolu vX.Y.Z

Released: YYYY-MM-DD

## Highlights

<2-3 sentence summary>

## Upgrade notes

<user-facing upgrade steps, e.g. `/plugin install foo@toolu` — delete this section if none>

## Included changes since v<prev>

- `<plugin>`: <change description>. (`<plugin>` <old-ver> → <new-ver>)
- …
```

### 3. Commit, tag, push

```sh
git add -A
git commit -m "chore(release): v<version>"
git tag -a v<version> -m "v<version> — see docs/releases/v<version>.md"
git push --follow-tags
```

The tag annotation points reviewers at the curated notes (the same file that ships as the GitHub Release body in step 5).

### 4. CI gate: tag must match manifest

CI's `release.yml` **verifies** that `git tag (vX.Y.Z)` matches `plugins/toolu/.claude-plugin/plugin.json` version. If they diverge, the release is blocked. This is the safety net for the "stale version" problem.

### 5. GitHub Release

On tag push, `release.yml` runs `gh release create`. If `docs/releases/v<version>.md` exists at the tagged commit, it is uploaded as the release body via `--notes-file`. For older tags that predate the file, the workflow falls back to `--generate-notes`. The tag gate (step 4) must pass first.

## CI Pipeline

All workflows live in `.github/workflows/`:

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `tests.yml` | push/PR to `main` | Typecheck (bun), bats suite, colocated-test layout enforcement, deterministic benchmarks, context-budget guard |
| `release.yml` | tag `v*.*.*` | Tag→manifest version check, `gh release create` (uses `docs/releases/v<tag>.md` as the release body when present) |
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
| `tooling/release.sh` | Atomic version bump for the monorepo (`--dry-run`, `--no-notes`, auto-drafts `docs/releases/v<ver>.md`) |
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
