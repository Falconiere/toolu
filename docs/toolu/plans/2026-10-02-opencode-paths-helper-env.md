# OpenCode paths, helper environment and concurrent state (OP-09) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-paths-helper-env-design.md   **Topic:** A global config root and keyed per-project data roots; HOME and foreign host roots kept out of toolu's processes; non-secret helper environment through `shell.env`; per-plugin roots in generated surfaces.

## Evidence and approach

**Recalled.**
- `6990a41a`: the OP-01 host facts and the headless driving rules: closed stdin, `PWD`, the scripted provider.
- OP-08 spec non-goal 2: HOME, `TOOLU_PLUGIN_ROOT`, `shell.env` and shared roots are deferred here.

**Inspected.**
- `tools/toolu-opencode/src/host/{roots,types,index}.ts`.
- `bootstrap/runtime.ts`: `bootstrapEnv` sets `HOME=dataRoot`; Bun is resolved with the original HOME.
- `plugin/{enforcement,hooks,context}.ts`.
- `adapter/evaluate.ts`: its env spreads `process.env` back over everything.
- `preflight/check.ts` (`resolveBunExecutable`).
- Core: `config/config-files.ts` (the only user-config reader), `config/settings-dir.ts` (the `~/.claude/settings` fallback), `host/host-roots.ts` (`configRoot`, `pluginRoot`, `pluginData`), `registry/registry-gate.ts` (Claude only) and `startup/publish.ts`.
- `scripts/lib/{rewrite,emit,constants}.ts`: the single `${TOOLU_PLUGIN_ROOT}` rewrite and package paths.
- `tooling/src/opencode-host/{session,scenarios-entry,scenarios-startup,host-run}.ts` and `tooling/src/opencode-entry-smoke.ts`.
- `bootstrap/__tests__/fixtures.ts` (`fixturePlugin`, `tempRoot`).
- Core `state/__tests__/gate-file-concurrency.test.ts`.
- Live probe on the pinned host (brainstorm): a linked worktree's `worktree` is itself, and `shell.env` reaches bash.

**Approach.** The spec's architecture.
- `host/roots.ts` resolves the global and data roots.
- A new `host/runtime-env.ts` builds every toolu environment: startup children, gates and `shell.env`.
- The core gets one variable, `TOOLU_USER_CONFIG_DIR`, in `config-files.ts`. Its bundles are rebuilt.
- The generator rewrites per-plugin roots.
- The live harness gains two scenarios, and the smoke runner gains an optional scenario filter so they can run alone.

## Workstream summary

1. Core user-config variable, then a bundle rebuild.
2. OpenCode roots, then the runtime-env builder.
3. Bootstrap env, then the gate env, enforcement and hooks.
4. Generator and regenerated surface.
5. Live scenarios.
6. Docs, then the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "core-user-config",
    "title": "packages/toolu-core/src/config/config-files.ts: files.user honors non-empty TOOLU_USER_CONFIG_DIR, else <configRoot>/toolu.config.json; new colocated config-files.test.ts covering Claude/Codex/OpenCode unset parity and the override, including loadConfig reading a real user file from that dir",
    "ac_refs": [
      "AC-2",
      "AC-10"
    ],
    "paths": [
      "packages/toolu-core/src/config/config-files.ts",
      "packages/toolu-core/src/config/__tests__/config-files.test.ts",
      "packages/toolu-core/src/config/__tests__/config-load.test.ts"
    ],
    "input": "Temp config roots with real toolu.config.json files; env maps for claude, codex and opencode hosts with and without TOOLU_USER_CONFIG_DIR",
    "check": "bun test --timeout 60000 packages/toolu-core/src/config/__tests__/config-files.test.ts packages/toolu-core/src/config/__tests__/config-load.test.ts",
    "model": "inherit"
  },
  {
    "id": "bundles",
    "title": "Rebuild the plugin bundles that embed config-files.ts (bun run build:plugins) and confirm no other drift",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "core-user-config"
    ],
    "paths": [
      "plugins/*/hooks/dist/**",
      "plugins/*/hooks/src/**",
      "packages/toolu-core/src/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "Committed plugin sources and the pinned Bun 1.4.2",
    "check": "bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "roots",
    "title": "tools/toolu-opencode/src/host/roots.ts: opencodeConfigRoot = global root (TOOLU_CONFIG_DIR \u2192 TOOLU_OPENCODE_HOME \u2192 XDG_CONFIG_HOME/opencode \u2192 HOME/.config/opencode, never throws); opencodeDataRoot = explicit \u2192 <override>/toolu/opencode/projects/<key> \u2192 <project>/.opencode/toolu/state; opencodeProjectKey (slug + 16 hex of sha256(realpath|resolve)); export from host/index.ts; host.test.ts",
    "ac_refs": [
      "AC-1"
    ],
    "paths": [
      "tools/toolu-opencode/src/host/roots.ts",
      "tools/toolu-opencode/src/host/types.ts",
      "tools/toolu-opencode/src/host/index.ts",
      "tools/toolu-opencode/src/host/__tests__/host.test.ts"
    ],
    "input": "Real temp git repos A and B, a linked worktree of A created with git worktree add (space in its path), a symlink to A, a not-yet-existing path; env maps with each override",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/host/__tests__/host.test.ts",
    "model": "inherit"
  },
  {
    "id": "runtime-env",
    "title": "tools/toolu-opencode/src/host/runtime-env.ts: OpencodeRoots, FOREIGN_HOST_VARS, withoutForeignHostVars, tooluProcessEnv, pluginRootVar, shellEnvAdditions (per-selected-plugin roots, TOOLU_PLUGIN_ROOT = core, TOOLU_OPENCODE_ROOT, TOOLU_BUN, PATH appended only when bun is absent); runtime-env.test.ts",
    "ac_refs": [
      "AC-1",
      "AC-5"
    ],
    "depends_on": [
      "roots"
    ],
    "paths": [
      "tools/toolu-opencode/src/host/runtime-env.ts",
      "tools/toolu-opencode/src/host/index.ts",
      "tools/toolu-opencode/src/host/__tests__/runtime-env.test.ts",
      "plugins/*/.claude-plugin/plugin.json"
    ],
    "input": "Real catalog manifests for toolu, context7 and epic-orchestrator (selectPluginsWithDependencies); a host env with EXA_API_KEY, CLAUDE_CONFIG_DIR, CODEX_HOME, PLUGIN_ROOT and a stray TOOLU_PROJECT_DIR; PATH with and without a real bun directory",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/host/__tests__/runtime-env.test.ts",
    "model": "inherit"
  },
  {
    "id": "bootstrap-env",
    "title": "bootstrap/runtime.ts: env from tooluProcessEnv (real HOME unless isolatedHome; foreign vars removed; TOOLU_USER_CONFIG_DIR, TOOLU_SETTINGS_DIR, TOOLU_BUN), per-entry CLAUDE_PLUGIN_ROOT and TOOLU_PLUGIN_ROOT, Bun from the host env, userConfigRoot option; new startup-environment.test.ts (AC-4) and startup-isolation.test.ts (AC-8); existing bootstrap tests stay green",
    "ac_refs": [
      "AC-4",
      "AC-8"
    ],
    "depends_on": [
      "bundles",
      "runtime-env"
    ],
    "paths": [
      "tools/toolu-opencode/src/bootstrap/**",
      "tools/toolu-opencode/src/host/**",
      "tools/toolu-opencode/src/preflight/check.ts",
      "plugins/*/hooks/dist/**",
      "plugins/*/hooks/hooks.json"
    ],
    "input": "All 16 real catalog plugins plus a fixture plugin whose startup entry prints HOME, every FOREIGN_HOST_VARS key and the TOOLU_* roots as SessionStart context; env HOME = a fake home holding .claude/settings, .claude/toolu.config.json and .codex/ (no isolatedHome); CLAUDE_CONFIG_DIR/CODEX_HOME/PLUGIN_ROOT/CLAUDE_PLUGIN_DATA pointing at poisoned temp dirs; path+bytes+mtime snapshots of those trees before and after; projects A {toolu, ast-grep} and B {toolu} sharing one TOOLU_CONFIG_DIR started A then B; two concurrent bootstraps of one project",
    "check": "bun test --timeout 300000 tools/toolu-opencode/src/bootstrap/__tests__",
    "model": "inherit"
  },
  {
    "id": "gate-env",
    "title": "adapter/evaluate.ts builds its env from opts.env (default process.env) minus foreign vars, plus TOOLU_USER_CONFIG_DIR when userConfigRoot is given; plugin/enforcement.ts resolves OpencodeRoots once (package root, global root, data root), passes them to bootstrap and gates, and returns shellEnv; plugin/hooks.ts adds shell.env on ready only; tests: hooks.test.ts (AC-5), new roots-config.test.ts (AC-3, AC-11), existing adapter/plugin tests",
    "ac_refs": [
      "AC-3",
      "AC-5",
      "AC-11"
    ],
    "depends_on": [
      "bootstrap-env"
    ],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**",
      "tools/toolu-opencode/src/host/**",
      "tools/toolu-opencode/src/bootstrap/**",
      "packages/toolu-core/src/config/**",
      "packages/toolu-core/src/gates/**",
      "plugins/toolu/settings/**"
    ],
    "input": "Temp git repos with a protected .env and one commit; global toolu.config.json under a temp XDG_CONFIG_HOME/opencode (protectedFiles block), a project .opencode/toolu.config.json (protectedFiles off), a TOOLU_CONFIG_DIR config (block); repo A with a failing .opencode/tmp/quality-gate-status.json and repo B without: each repo's real prepareEnforcement hook (real toolu bootstrap) receives the same bash git commit -m x call: A throws the quality-gate reason, B resolves, A's gate file bytes are unchanged; core's real-process concurrent gate writers; a ready vs not-ready createTooluHooks for shell.env presence",
    "check": "bun test --timeout 300000 tools/toolu-opencode/src/plugin/__tests__ tools/toolu-opencode/src/adapter/__tests__ packages/toolu-core/src/state/__tests__/gate-file-concurrency.test.ts",
    "model": "inherit"
  },
  {
    "id": "generator",
    "title": "scripts/lib: ${CLAUDE_PLUGIN_ROOT} in plugin X \u2192 ${TOOLU_PLUGIN_ROOT_<X>}; package generated paths \u2192 ${TOOLU_OPENCODE_ROOT}; GENERATED-NOTES states the env contract; epic-orchestrator SKILL.md comment; bun run generate:opencode-surface; generate-surface.test.ts",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "runtime-env"
    ],
    "paths": [
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/*/skills/**",
      "plugins/*/agents/**",
      "plugins/*/commands/**",
      "docs/**"
    ],
    "input": "The repo's 16 plugin sources and docs",
    "check": "bun test --timeout 120000 tools/toolu-opencode/scripts/__tests__ && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "tooling/src/opencode-host: new scenarios-paths.ts with entry.helper-env (AC-6: spaces in project and catalog paths, PATH without bun, generated context7 helper line, plan-ledger, per-plugin root, HOME) and entry.worktree-state (AC-7: main {toolu, ast-grep} with failing gate, linked worktree {toolu}); register in opencode-entry-smoke.ts with an optional scenario-id filter; run against the pinned host",
    "ac_refs": [
      "AC-6",
      "AC-7"
    ],
    "depends_on": [
      "gate-env",
      "generator"
    ],
    "paths": [
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-entry-smoke.ts",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/skills/context7-context7/SKILL.md",
      "plugins/**"
    ],
    "input": "Pinned opencode-ai@1.18.34 from ~/.cache/toolu/opencode-host, isolated profiles, scripted loopback provider. entry.helper-env: project dir and catalog copy (TOOLU_REPO_ROOT) both under paths with spaces, PATH=/usr/bin:/bin without bun and TOOLU_BUN set, toolu+context7+epic-orchestrator enabled; scripted bash runs the generated context7 SKILL.md OpenCode helper line with --help, bun \"$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js\" --help, test -f \"$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts\", printf %s \"$HOME\"; each writes a marker file checked on disk. entry.worktree-state: repo M (toolu+ast-grep, failing .opencode/tmp/quality-gate-status.json) and linked worktree W at a path with a space (toolu only); M session scripts git commit, W session starts; checks: M commit tool state is error with the quality-gate reason, M gate file bytes equal before/after, both M ast-grep modules still present after W's startup, W/.opencode/toolu/state exists without ast-grep modules",
    "check": "bun run smoke:opencode-entry entry.helper-env entry.worktree-state && bun test --timeout 60000 tooling/src/opencode-host/__tests__",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/opencode.md (roots table, shell.env variables and Bun PATH rule, HOME kept, keyed override roots replacing the shared-root caveat, old-file cleanup, nested-host caveat, live scenarios), docs/config.md (TOOLU_USER_CONFIG_DIR, OpenCode locations), docs/portable-core.md (host row, rewrite sentence); regenerate the surface mirror",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "live"
    ],
    "paths": [
      "docs/opencode.md",
      "docs/config.md",
      "docs/portable-core.md",
      "tools/toolu-opencode/generated/**",
      "tooling/src/check-portable-core-doc.ts"
    ],
    "input": "The documents and the regenerated mirror",
    "check": "bun run generate:opencode-surface && bun run check:opencode-surface && bun run test:portable-core",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "docs"
    ],
    "paths": [
      "**"
    ],
    "input": "The whole repository",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- Create:
  - `packages/toolu-core/src/config/__tests__/config-files.test.ts`
  - `tools/toolu-opencode/src/host/runtime-env.ts`, `tools/toolu-opencode/src/host/__tests__/runtime-env.test.ts`
  - `tools/toolu-opencode/src/bootstrap/__tests__/startup-environment.test.ts`, `startup-isolation.test.ts`
  - `tools/toolu-opencode/src/plugin/__tests__/roots-config.test.ts`
  - `tooling/src/opencode-host/scenarios-paths.ts`
- Modify:
  - `packages/toolu-core/src/config/config-files.ts`
  - `tools/toolu-opencode/src/host/{roots,types,index}.ts`
  - `tools/toolu-opencode/src/bootstrap/runtime.ts`
  - `tools/toolu-opencode/src/adapter/evaluate.ts`
  - `tools/toolu-opencode/src/plugin/{enforcement,hooks}.ts`
  - `tools/toolu-opencode/scripts/lib/{rewrite,emit,constants,render}.ts`, `tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts`
  - `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`
  - `tooling/src/opencode-entry-smoke.ts`
  - `docs/{opencode,config,portable-core}.md`
- Regenerate: `plugins/*/hooks/dist/*.js`, `tools/toolu-opencode/generated/**`

## Verification

- **End to end:** the live scenarios (AC-6, AC-7) on the pinned host prove the following with real files and real bash:
  - helpers resolve from paths with spaces and without Bun on `PATH`;
  - HOME stays the user's own;
  - worktree data roots stay separate;
  - a project's gate state is read, not consumed.
- **Hermetic real-subprocess tests** prove the root precedence, the global/project config layering through real gates, foreign-home immunity with byte and mtime snapshots, shared-override isolation, concurrent startup and two-project gate isolation.
- **Failure and boundary cases:**
  - a missing project path for the key;
  - a stray `TOOLU_PROJECT_DIR`;
  - `bun` on and off `PATH`;
  - a not-ready instance with no `shell.env`;
  - poisoned Claude/Codex homes.
- **Docs:** the opencode, config and portable-core docs and the regenerated mirror. `bun run test` is the final gate.
- **Delivery:**
  1. Scoped `feat(opencode)` commit.
  2. `bun plugins/toolu/hooks/dist/plan-ledger.js run <plan> --verify`.
  3. `toolu-review:review` with version 2 push-review state.
  4. `verdict.js status` reports `overall: ready`.
  5. Rebase on `origin/main` if it moved, then push.
  6. A PR against `main` starting `Closes Falconiere/toolu#343` / `Part of Falconiere/toolu#334`, then `pr-babysit:babysit`.

## Plan review

Round 1, **Status:** Needs changes.

- Every AC from AC-1 to AC-11 maps to at least one step's `ac_refs`, and every step has a runnable check.
- Jev step/requirement alignment (`noul`): AC-4 0.74, AC-6 0.80, AC-7 0.69, AC-11 0.63.
- live: 🟡 should-fix (fixed): the input did not name AC-7's observables (gate-file bytes, M modules after W, W's root). They are now spelled out per scenario.
- gate-env: 🟡 should-fix (fixed): the input did not say both repos' real hooks receive the same commit call. Now it does.
- bootstrap-env: 🔵 consider (fixed): the snapshot method (path, bytes, mtime) is now named.
- Verification: 🟡 should-fix (fixed): the delivery sequence (ledger verify, review, verdict, rebase, PR, babysit) was missing. Added.
- generator: 🔵 consider: import `pluginRootVar` from `src/host/runtime-env.ts` so the generator and `shell.env` share one naming rule.

Round 2, **Status:** Approved. Jev: AC-4 0.77, AC-6 0.83, AC-7 0.76, AC-11 0.85. The dependency order is acyclic: core → bundles → roots → runtime-env → bootstrap → gates → generator → live → docs → gate.
