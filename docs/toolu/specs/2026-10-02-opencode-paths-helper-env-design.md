# OpenCode paths, helper environment and concurrent state — Design

**Date:** 2026-10-02   **Status:** Draft   **Author:** Claude (epic worker, #343)   **Topic:** Resolve OpenCode's global config root and per-project data roots, keep HOME and foreign host homes out of toolu's runtime, and give bash helpers a resolvable environment through `shell.env` (OP-09)

## Problem

On OpenCode, toolu's roots and helper environment are wrong in four ways:

- **HOME is replaced.** `bootstrapEnv` (`tools/toolu-opencode/src/bootstrap/runtime.ts`) sets every startup child's `HOME` to the data root. Inherited `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PLUGIN_ROOT` and `CLAUDE_PLUGIN_DATA` still pass through to children and in-process gates. Without `TOOLU_SETTINGS_DIR`, core `settingsDir` falls back to `~/.claude/settings`.
- **No global config.** The adapter forces core `TOOLU_CONFIG_DIR` to the data root, which is per project by default. Core then reads the user-level `toolu.config.json` from the data root, so nothing in OpenCode's global config directory (`${XDG_CONFIG_HOME:-~/.config}/opencode`) is ever read.
- **A shared override corrupts.** With `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME` set, every project shares one data root. The last project to start prunes the others' registry modules and helpers, and relinks helpers to its own package version. `docs/opencode.md` defers this to #343.
- **Helpers do not resolve.** The agent's bash has no toolu variables. Generated skills name helpers as `"${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/<plugin>/<helper>"`, but startup published them under `<project>/.opencode/toolu/state`. Helper bundles run through `#!/usr/bin/env bun`, which fails when Bun is not on `PATH`. The generator also rewrites every plugin's `${CLAUDE_PLUGIN_ROOT}` to a single `${TOOLU_PLUGIN_ROOT}`, which already has three meanings:
  - epic-orchestrator: its own root;
  - delivery-flow and core `pluginRoot()`: the toolu core plugin;
  - `GENERATED-NOTES.md` and the generated rubric link: the `@toolu/opencode` package root.

Live evidence on the pinned host (`opencode-ai@1.18.34`, scratch plugin under the probe harness, brainstorm `docs/toolu/brainstorms/2026-10-02-opencode-paths-helper-env.md`):

- In a linked git worktree whose path has a space, `PluginInput.directory`, `worktree` and `project.worktree` are all the linked worktree root.
- `shell.env` receives `{cwd, sessionID, callID}` and an empty `output.env`, and the keys set there reach the bash tool.

## Non-Goals

1. Context delivery (OP-07, #341), surface installation and discovery (OP-11, #345), and post-tool dispatch (OP-06, #340).
2. Leaf scripts that hard-code Claude/Codex homes when the agent runs them by hand: epic-orchestrator `scripts/common.ts`, `route.ts` and `trackers/jira.ts` (OP-22), pr-babysit `fixer-route.ts` (OP-23), and statusline `settings.ts`/`collect.ts` (OP-25). This change gives them the environment their ports will use.
3. Changing Claude Code or Codex behavior. The one core change is a new variable that only the OpenCode adapter sets.
4. Moving the default data root. It stays `<project>/.opencode/toolu/state` (OP-08). Old shared-override files are not migrated; they become inert and the docs say how to delete them.
5. Removing a parent session's variables from a process that the agent itself starts from bash (another `opencode`, `claude` or `codex`). This is documented as a caveat.

## Architecture

**Roots** (`tools/toolu-opencode/src/host/roots.ts`):

| Root | Resolution | Holds |
|---|---|---|
| project | host `worktree`, or `directory` when `worktree` is `/` (unchanged) | `.opencode/toolu.config.json`, `.opencode/toolu/plugins.json`, gate state `.opencode/tmp/` |
| global config (`opencodeConfigRoot`) | `TOOLU_CONFIG_DIR` → `TOOLU_OPENCODE_HOME` → `$XDG_CONFIG_HOME/opencode` → `$HOME/.config/opencode` (core `configRoot({host:"opencode"})`) | global `toolu.config.json`; read-only to toolu |
| data (`opencodeDataRoot`) | explicit `dataRoot` → override set: `<override>/toolu/opencode/projects/<key>` → `<project>/.opencode/toolu/state` | registry modules, helpers, startup ledger |

Where:

- `<override>` is the first non-empty value of `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME`.
- `<key>` is `<slug>-<sha256(realpath(project))[0..16]>`, with `slug` = the lowercased project basename, non-`[a-z0-9]` runs turned into `-`, trimmed to 32 characters.
- `toolu/opencode/projects/` sits beside Claude/Codex's `toolu/{pre,post}-tools.d` when one `TOOLU_CONFIG_DIR` is shared across hosts. No host's prune touches it.

**One runtime environment builder** (`tools/toolu-opencode/src/host/runtime-env.ts`) serves startup children, in-process gates and `shell.env`.

`tooluProcessEnv(base, roots)` produces the environment for toolu's own startup children and gates:

- Start from the host env with `FOREIGN_HOST_VARS` removed. That list is `CLAUDE_CONFIG_DIR`, `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, `CLAUDE_PLUGINS_REGISTRY`, `CODEX_HOME`, `PLUGIN_ROOT`, `PLUGIN_DATA`, `TOOLU_CODEX_PLUGIN_SNAPSHOT`, `CURSOR_PROJECT_DIR`, `CURSOR_PLUGIN_ROOT` and `HERMES_HOME`.
- Then set `TOOLU_CONFIG_DIR=<data>`, `TOOLU_USER_CONFIG_DIR=<global>`, `TOOLU_PROJECT_DIR=<project>`, `TOOLU_PROJECT_CONFIG_DIRNAME=.opencode`, `TOOLU_HOST_OVERRIDE=opencode` and `TOOLU_SETTINGS_DIR=<repo>/plugins/toolu/settings`.
- `HOME` is the host's own value. The `isolatedHome` bootstrap option remains a test-only override.

`shellEnvAdditions(...)` produces the non-secret variables that `shell.env` adds to every bash call:

- `TOOLU_HOST_OVERRIDE`, `TOOLU_CONFIG_DIR=<data>`, `TOOLU_USER_CONFIG_DIR`, `TOOLU_PROJECT_CONFIG_DIRNAME` and `TOOLU_SETTINGS_DIR`.
- `TOOLU_BUN=<resolved Bun>`.
- `TOOLU_OPENCODE_ROOT=<@toolu/opencode package root>`, which holds `generated/`.
- `TOOLU_PLUGIN_ROOT=<toolu core plugin dir>`, the meaning core `pluginRoot()` and delivery-flow already use.
- `TOOLU_PLUGIN_ROOT_<NAME>=<plugin dir>` for each **selected** plugin. The name is uppercased and `-` becomes `_`.
- `PATH=<PATH><delimiter><dir of resolved Bun>`, only when `bun` is not found on the host `PATH`.

It never adds `HOME`, `TOOLU_PROJECT_DIR`, or any variable copied from the host env, so secrets such as API keys reach bash only as the host already passes them. `TOOLU_PROJECT_DIR` stays out so that a helper run in another repository resolves that repository's state from its own git toplevel.

**Core** (`packages/toolu-core/src/config/config-files.ts`): `files.user` is `<TOOLU_USER_CONFIG_DIR>/toolu.config.json` when that variable is non-empty, and `<configRoot>/toolu.config.json` otherwise. No host sets it except the OpenCode adapter, so Claude and Codex resolve exactly as before.

**Wiring:**

- `prepareEnforcement` resolves all roots once from the binding.
- `bootstrapRuntime` builds its env with `tooluProcessEnv`. Each entry adds `CLAUDE_PLUGIN_ROOT` and `TOOLU_PLUGIN_ROOT` = that plugin's dir, plus `TOOLU_STARTUP_REPORT`. Bun is resolved from the host env (TOOLU_BUN → PATH → `~/.bun/bin/bun`).
- `createGateDecider` (`adapter/evaluate.ts`) stops spreading `process.env` over the scrubbed env. It builds its env from `opts.env` (default `process.env`) minus foreign vars, and passes `TOOLU_USER_CONFIG_DIR` when `userConfigRoot` is given.
- `createTooluHooks` returns `"shell.env"` beside `tool.execute.before` only when enforcement is ready.

**Generator** (`tools/toolu-opencode/scripts/lib`):

- `${CLAUDE_PLUGIN_ROOT}` in plugin X's surfaces becomes `${TOOLU_PLUGIN_ROOT_X}`.
- Package-relative generated paths use `${TOOLU_OPENCODE_ROOT}/generated/...` instead of `${TOOLU_PLUGIN_ROOT}`.
- `GENERATED-NOTES.md` states the environment contract instead of "set `TOOLU_PLUGIN_ROOT` to the package root".
- The epic-orchestrator source comment names the per-plugin variable.

## Interfaces / Schema

```ts
// tools/toolu-opencode/src/host/roots.ts
export function opencodeConfigRoot(options?: OpencodeRootsOptions): string;   // global; never throws
export function opencodeDataRoot(options?: OpencodeRootsOptions): string;     // throws without projectRoot unless dataRoot given
export function opencodeProjectKey(projectRoot: string): string;              // `<slug>-<16 hex>`

// tools/toolu-opencode/src/host/runtime-env.ts
export type OpencodeRoots = {
  projectRoot: string; dataRoot: string; userConfigRoot: string;
  repoRoot: string;    // catalog parent: <repoRoot>/plugins
  packageRoot: string; // @toolu/opencode package dir (holds generated/)
};
export const FOREIGN_HOST_VARS: readonly string[];
export function withoutForeignHostVars(env: Record<string, string>): Record<string, string>;
export function tooluProcessEnv(base: Record<string, string>, roots: OpencodeRoots): Record<string, string>;
export function pluginRootVar(name: string): string;  // "epic-orchestrator" → "TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR"
export function shellEnvAdditions(input: {
  roots: OpencodeRoots; plugins: readonly PluginManifest[]; bun: string; hostPath: string | undefined;
}): Record<string, string>;

// bootstrap/runtime.ts: BootstrapRuntimeOptions gains `userConfigRoot?: string` (default opencodeConfigRoot({env}))
// adapter/evaluate.ts: PermissionEvaluateHandlerOptions gains `userConfigRoot?: string`
// plugin/enforcement.ts: Enforcement ready variant gains `shellEnv: Record<string, string>`
```

Hooks shape on a ready instance: `{ "tool.execute.before", "shell.env": (input, output) => { Object.assign(output.env, shellEnv) }, dispose }`.

## Failure modes and edge cases

| Case | Behavior |
|---|---|
| No `projectRoot` and no `dataRoot` | `opencodeDataRoot` throws (unchanged contract); `prepareEnforcement` always has a project, so the throw becomes a not-ready deny-all |
| `HOME` and `XDG_CONFIG_HOME` both unset | Global root falls back to `os.homedir()/.config/opencode` (core `home()`) |
| Global config absent or malformed | Absent: defaults. Malformed: core's existing fail-closed envelope. Unchanged |
| Two projects or worktrees share an override | Distinct keyed data roots, so neither prunes or relinks the other's contributions |
| The same project from a symlinked path | The key hashes the real path, so both paths map to one data root |
| Two sessions start at once in one project | Same data root and selection. Writes are atomic renames (OP-08), so both are ready and the ledger stays valid |
| `bun` already on PATH | `PATH` is not added. A PATH bun that differs from `TOOLU_BUN` is used by `env bun` helpers, as on other hosts |
| No Bun resolvable | Preflight not-ready (unchanged); bash is denied, so `shell.env` is never installed |
| `shell.env` called when not ready | Not installed |
| User sets `TOOLU_CONFIG_DIR` | Global root = that dir. Bash sees `TOOLU_CONFIG_DIR=<keyed data root>` and `TOOLU_USER_CONFIG_DIR=<that dir>` (documented) |
| A nested host started from the agent's bash | It inherits the injected variables. Documented: unset `TOOLU_*` first |

## Acceptance criteria

- **AC-1:** Given env and project inputs, `opencodeConfigRoot` resolves `TOOLU_CONFIG_DIR`, then `TOOLU_OPENCODE_HOME`, then `$XDG_CONFIG_HOME/opencode`, then `$HOME/.config/opencode`. `opencodeDataRoot` gives a project without an override `<project>/.opencode/toolu/state`. Two different projects, and a linked worktree of one of them, under one `TOOLU_CONFIG_DIR` get three distinct `<override>/toolu/opencode/projects/<key>` roots. The same project via a symlink gets the same root.
- **AC-2:** With `TOOLU_USER_CONFIG_DIR=/g` and `TOOLU_CONFIG_DIR=/d`, core `configFiles` returns user `/g/toolu.config.json`. With it unset, the user path stays `<configRoot>/toolu.config.json` on Claude, Codex and OpenCode.
- **AC-3:** Through real `prepareEnforcement` (toolu selected, real bootstrap) on a project with a protected `.env`:
  - a global `$XDG_CONFIG_HOME/opencode/toolu.config.json` setting `protectedFiles` to `block` denies a write to `.env`;
  - adding a project `.opencode/toolu.config.json` with `protectedFiles` `off` allows it;
  - with `TOOLU_CONFIG_DIR=<dir>` holding the `block` config instead, the write is denied again.
- **AC-4:** A real bootstrap of all 16 catalog plugins, plus a fixture plugin whose startup entry reports its environment, runs with a fake `HOME` (no `isolatedHome`) that contains `.claude/settings/`, `.claude/toolu.config.json` and `.codex/`, and with `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PLUGIN_ROOT` and `CLAUDE_PLUGIN_DATA` pointing at further poisoned directories. It is ready. The entry sees `HOME` unchanged, none of `FOREIGN_HOST_VARS`, and the toolu roots. Every file under the fake HOME's `.claude` and `.codex` and under the poisoned directories keeps its path, bytes and mtime, and nothing new appears.
- **AC-5:** `shellEnvAdditions` for selection {toolu, context7, epic-orchestrator} returns exactly the specified keys:
  - `TOOLU_PLUGIN_ROOT_*` only for those three, and `TOOLU_PLUGIN_ROOT` equal to `TOOLU_PLUGIN_ROOT_TOOLU`;
  - `PATH` added only when the given host `PATH` lacks `bun`, with the Bun dir appended last;
  - no key copied from a host env containing `EXA_API_KEY`.

  A ready `createTooluHooks` exposes `shell.env`, which fills `output.env` with those additions. A not-ready one has no `shell.env`.
- **AC-6:** On the pinned live host, with project and catalog paths containing spaces, `PATH` without Bun (`TOOLU_BUN` set), and toolu, context7 and epic-orchestrator enabled, scripted bash calls succeed:
  - the generated context7 skill's own helper command, run with `--help`, prints the Context7 usage;
  - `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" --help` exits 0;
  - `test -f "$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts"` succeeds;
  - `printf %s "$HOME"` equals the profile HOME.
- **AC-7:** On the pinned live host, a repository with a linked worktree is set up with a failing quality gate recorded only in the main checkout's `.opencode/tmp/quality-gate-status.json`. A scripted `git commit` is denied by the quality gate in the main checkout session and runs in the linked worktree session. The main checkout's gate file is byte-identical afterwards.
- **AC-8:** Real bootstraps:
  - Projects A (toolu, ast-grep) and B (toolu) share one `TOOLU_CONFIG_DIR` and start A, then B. Both are ready, and A's keyed data root still holds both ast-grep modules byte-equal to the bundles.
  - Two bootstraps of one project started concurrently both return ready, and the resulting ledger parses.
- **AC-9:** Regenerating the surface gives:
  - `${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR}` in the epic-orchestrator skill;
  - `${TOOLU_OPENCODE_ROOT}/generated/...` for package paths;
  - no bare `${TOOLU_PLUGIN_ROOT}` token produced by a rewrite;
  - notes stating the environment contract.

  `bun run check:opencode-surface` passes.
- **AC-10:** Claude Code and Codex are unchanged: `bun run test` passes with no fixture or golden capture changed for those hosts.

## Acceptance evidence

| AC | Real input / fixture | Expected result | Check |
|---|---|---|---|
| AC-1 | Temp dirs: two git projects, a linked worktree, a symlink | Exact paths; distinct keys; stable for the symlink | `bun test tools/toolu-opencode/src/host/__tests__/host.test.ts` |
| AC-2 | Temp config dirs and `configFiles` | User path per variable; unchanged when unset | `bun test packages/toolu-core/src/config/__tests__/config-files.test.ts` |
| AC-3 | Temp git project with `.env`, real toolu bootstrap, global/project config files | deny / allow / deny | `bun test tools/toolu-opencode/src/plugin/__tests__/roots-config.test.ts` |
| AC-4 | Full catalog plus fixture env-reporting plugin; poisoned fake HOME and env dirs | Ready; env as specified; snapshots equal | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-environment.test.ts` |
| AC-5 | Real catalog manifests; host env with a secret | Exact key set; `PATH` rule; hook wiring | `bun test tools/toolu-opencode/src/host/__tests__/runtime-env.test.ts tools/toolu-opencode/src/plugin/__tests__/hooks.test.ts` |
| AC-6 | Pinned `opencode-ai@1.18.34`, isolated profile, scripted provider | Marker files with the expected contents | `bun run smoke:opencode-entry` (`entry.helper-env`) |
| AC-7 | Pinned host, repository with a linked worktree, failing gate in main | main: commit denied; worktree: commit made; gate file unchanged | `bun run smoke:opencode-entry` (`entry.worktree-state`) |
| AC-8 | Repo catalog bundles, temp projects and a shared override | Ready; modules intact; ledger valid | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-isolation.test.ts` |
| AC-9 | Repo plugin sources | Rewritten tokens; drift check clean | `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts && bun run check:opencode-surface` |
| AC-10 | Whole repo | Green | `bun run test` |

## Documentation impact

- `docs/opencode.md`:
  - roots table (global config, data root, keyed override roots), replacing the "Shared data root" caveat;
  - the `shell.env` variables and the Bun `PATH` rule;
  - HOME stays the user's own;
  - cleanup of old shared-override files;
  - the nested-host caveat.
- `docs/config.md`: `TOOLU_USER_CONFIG_DIR` under Locations, plus OpenCode locations.
- `docs/portable-core.md`: the `@toolu/opencode/host` row and the `${CLAUDE_PLUGIN_ROOT}` rewrite sentence.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`: comment line.
- `tools/toolu-opencode/generated/**`: regenerated, including `GENERATED-NOTES.md` and the mirrored docs.
- No README change: the install flow does not change.

## Open Questions

None blocking.

- Whether OP-05 (#339) turns the default `ask` into an advisory does not affect AC-3, which uses `block` and `off`.
- Whether leaf ports adopt `TOOLU_PLUGIN_ROOT_<NAME>` for their own scripts belongs to their owners. This change makes the variables available.
