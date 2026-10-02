# OpenCode install and discovery of generated surfaces — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-install-discover-surfaces-design.md   **Topic:** The `config` hook contributes the selected plugins' generated skills, agents and commands, with project/global selection, user-wins precedence and no duplicates (OP-11, #345)

## Evidence and approach

- **Recalled.** Memory `6990a41a` holds the OP-01 host contract: the `config` hook can inject command, agent and `skills.paths`, and the host loads invalid skill names. Memory `906b9b1a` covers the npm route, which takes `name@file:<tgz>`. Memory `de5f7462` records the host facts verified for this spec.
- **Host source** (pinned `v1.18.34`, read for the brainstorm and spec):
  - the config is merged before plugin init, and `config(cfg)` mutates that same object;
  - skills are scanned lazily, and same-name skills resolve nondeterministically;
  - permission evaluation is last match wins over [defaults, user, agent];
  - the host embeds Bun 1.3.14, which has `Bun.YAML`.
- **Live shape check** (scratch, pinned host): `debug skill` returns `{name, description, location, content}`; `GET /agent` and `GET /command` list the host's real services, and each command carries `source`.
- **Reused:**
  - `selectPluginsWithDependencies` and `resolveEnabledPluginNames` (`src/select`, `src/inventory`);
  - `opencodeConfigRoot` (`src/host/roots.ts`);
  - `claimInstance` (`src/plugin/once.ts`);
  - `createTooluHooks` and `startupNotes` (`src/plugin/hooks.ts`);
  - `prepareEnforcement` and `PACKAGE_ROOT` (`src/plugin/enforcement.ts`);
  - the live harness: `openSession`, `debugJson`, `withServe`, `runHost` and `toolStates`, and from `scenarios-entry.ts` `entrySession`, `packTarball`, `installShim` and `diagnostics`;
  - the generator notes in `scripts/lib/emit.ts`.
- **Design.** Exactly the spec's: the `config` hook applies a plan computed in `prepareEnforcement`, and no files are written.
- **Repository constraints:**
  - the guardrails folder tree must list `surfaces` (`tools/toolu-opencode/guardrails.config.json`), and functions stay at 60 lines or fewer;
  - jscpd runs on `tooling/src` and `tools/toolu-opencode/src` with zero tolerance (10 lines or 60 tokens), so code is kind-generic and the scenarios share helpers;
  - knip treats every `tools/toolu-opencode/src/**/*.ts` as an entry, but `tooling/src` exports must be used, so `scenarios-install.ts` exports only `INSTALL_SCENARIOS`.

## Workstream summary

selection (global file, fail closed) → catalog and plan (validated generated surfaces, guardrails tree) → skill-name mirror → apply (merge, permission, never throw) → wiring in enforcement and hooks → generator wording, regeneration and docs → live scenarios on the pinned host → full gate → delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "selection",
    "title": "inventory/selection.ts + select/resolve.ts + host/roots.ts: project file → global <configRoot>/toolu/plugins.json → all installed; an invalid explicit file → {ok:false, reason naming the path}; unknown names → notes; the result carries source; selectPluginsWithDependencies gains optional globalConfigRoot and returns source + notes (selectPluginsByEnabledNames: source default, notes []); opencodeGlobalPluginSelectionPath stays internal (no host/index.ts re-export); tests in inventory.test.ts and select.test.ts",
    "ac_refs": ["AC-6", "AC-7"],
    "paths": [
      "tools/toolu-opencode/src/inventory/selection.ts",
      "tools/toolu-opencode/src/select/resolve.ts",
      "tools/toolu-opencode/src/host/roots.ts",
      "tools/toolu-opencode/src/inventory/__tests__/inventory.test.ts",
      "tools/toolu-opencode/src/select/__tests__/select.test.ts"
    ],
    "input": "Real temp plugin roots with manifests; temp projects and temp global config roots holding: a valid global file only; a valid project file plus a global file (project wins); a project `enabled: []` plus a global file (empty project wins); an invalid global file while a project file exists (ignored); invalid project JSON, wrong version, extra key and non-string name (each not ready, naming the path); unknown names (note); neither file (all installed, source default)",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/inventory tools/toolu-opencode/src/select && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "catalog-plan",
    "title": "guardrails.config.json gains src/surfaces; surfaces/frontmatter.ts (canonical key: <JSON> parse; gray-matter-like name read: BOM, CRLF, Bun.YAML + colon sanitizer, duplicate keys = failure); surfaces/catalog.ts (Zod catalog, id rule, per-kind uniqueness, paths inside generatedDir, skill path shape, SKILL.md name = id); surfaces/plan.ts (catalog order, kind-generic agent/command entries, missing-plugin notes); colocated tests",
    "ac_refs": ["AC-7"],
    "depends_on": ["selection"],
    "paths": [
      "tools/toolu-opencode/guardrails.config.json",
      "tools/toolu-opencode/src/surfaces/frontmatter.ts",
      "tools/toolu-opencode/src/surfaces/catalog.ts",
      "tools/toolu-opencode/src/surfaces/plan.ts",
      "tools/toolu-opencode/src/surfaces/__tests__/frontmatter.test.ts",
      "tools/toolu-opencode/src/surfaces/__tests__/catalog.test.ts",
      "tools/toolu-opencode/src/surfaces/__tests__/plan.test.ts",
      "tools/toolu-opencode/generated/opencode.toolu.json",
      "tools/toolu-opencode/generated/skills/**",
      "tools/toolu-opencode/generated/agents/**",
      "tools/toolu-opencode/generated/commands/**"
    ],
    "input": "The committed tools/toolu-opencode/generated catalog, skills, agents and commands; temp copies with a missing file, a duplicate id, an escaping path, a renamed skill name, non-canonical agent frontmatter, an unknown agent key and invalid JSON; selections [pr-babysit, toolu] (catalog order), [jev], and a name absent from the catalog (note)",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/surfaces/__tests__/frontmatter.test.ts tools/toolu-opencode/src/surfaces/__tests__/catalog.test.ts tools/toolu-opencode/src/surfaces/__tests__/plan.test.ts && bun run guardrails && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "skill-names",
    "title": "surfaces/skill-names.ts: existingSkillNames mirrors the pinned host scan: HOME .claude/.agents skills with dot files; the .claude/.agents up-walk from directory to worktree, including a '/' worktree; config dirs (XDG opencode, the .opencode up-walk unless OPENCODE_DISABLE_PROJECT_CONFIG is truthy, ~/.opencode, OPENCODE_CONFIG_DIR) with {skill,skills}/** and no dot files; configured skills.paths with ~/ and relative resolution; Effect-boolean flags; symlinks followed with a realpath loop cut; the first location wins. Tests over temp trees",
    "ac_refs": ["AC-4", "AC-7"],
    "depends_on": ["catalog-plan"],
    "paths": [
      "tools/toolu-opencode/src/surfaces/skill-names.ts",
      "tools/toolu-opencode/src/surfaces/frontmatter.ts",
      "tools/toolu-opencode/src/surfaces/__tests__/skill-names.test.ts"
    ],
    "input": "Temp HOME, XDG_CONFIG_HOME, OPENCODE_CONFIG_DIR and nested git project trees with real SKILL.md files (valid, BOM, CRLF, quoted, duplicate-key, numeric name, under a dot directory, symlinked, looping symlink), each flag at true/1/yes/0/false/unset, and a subdirectory instance below the worktree",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/surfaces/__tests__/skill-names.test.ts",
    "model": "inherit"
  },
  {
    "id": "apply",
    "title": "surfaces/merge.ts (user-wins deepMerge) + surfaces/apply.ts: applySurfaces appends skill dirs to cfg.skills.paths except ids already defined; deep-merges agent/command entries under user entries (kind-generic); adds external_directory {<generatedDir>/resources/*: allow} only when no user permission key could match; validates cfg.skills; assigns each kind once; returns SurfaceReport. Tests over real config objects",
    "ac_refs": ["AC-3", "AC-7"],
    "depends_on": ["skill-names"],
    "paths": [
      "tools/toolu-opencode/src/surfaces/merge.ts",
      "tools/toolu-opencode/src/surfaces/apply.ts",
      "tools/toolu-opencode/src/surfaces/plan.ts",
      "tools/toolu-opencode/src/surfaces/skill-names.ts",
      "tools/toolu-opencode/src/surfaces/__tests__/apply.test.ts"
    ],
    "input": "Plans built from the committed generated tree; config objects with unrelated agents and commands, existing skills.paths and urls, partial and complete user entries, disable:true, permission objects with and without external_directory, with a string external_directory and with a wildcard key, and a malformed skills value",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/surfaces/__tests__/apply.test.ts && bun run typecheck && bun run knip && bun run jscpd",
    "model": "inherit"
  },
  {
    "id": "wiring",
    "title": "plugin/context.ts (HostBinding.worktree, raw host value); plugin/enforcement.ts (an injectable options object { findBundled, generatedDir }, the global selection root, selection notes into diagnostics, a realpath generatedDir plan, not ready on surface or selection failure, selectionSource); plugin/hooks.ts (a config hook only on the ready path: applySurfaces, surfaces and surface-notes log lines, catch-all error log). Update every HostBinding literal in tests",
    "ac_refs": ["AC-5", "AC-6", "AC-7"],
    "depends_on": ["apply"],
    "paths": [
      "tools/toolu-opencode/src/plugin/context.ts",
      "tools/toolu-opencode/src/plugin/enforcement.ts",
      "tools/toolu-opencode/src/plugin/hooks.ts",
      "tools/toolu-opencode/src/plugin/__tests__/hooks.test.ts",
      "tools/toolu-opencode/src/plugin/__tests__/context.test.ts",
      "tools/toolu-opencode/src/plugin/__tests__/toolu.test.ts",
      "tools/toolu-opencode/src/plugin/__tests__/roots-config.test.ts",
      "tools/toolu-opencode/src/plugin/__tests__/surfaces-hook.test.ts",
      "tools/toolu-opencode/src/surfaces/**",
      "tools/toolu-opencode/src/inventory/**",
      "tools/toolu-opencode/src/select/**"
    ],
    "input": "Real temp projects with selection files and the repository plugins/ catalog. createTooluHooks runs end to end (real bootstrap with process Bun), then its config hook is applied to real config objects. Boundaries: a corrupt temp generatedDir (not ready, no config hook), an invalid selection file (not ready), a second instance on the same directory (no config hook), and malformed config passed to the hook (returns normally, error log)",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin tools/toolu-opencode/src/surfaces && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Generator wording in scripts/lib/emit.ts + full bun run generate:opencode-surface; docs/opencode.md (automatic discovery, precedence, notes, permission, global selection, invalid-file failure, lifecycle, migration, surfaces.* in the live smoke section); docs/portable-core.md selection row; tools/toolu-opencode/README.md; host-contract OP-11 constraints via capability-matrix.json and --write-doc",
    "ac_refs": ["AC-8"],
    "depends_on": ["wiring"],
    "paths": [
      "tools/toolu-opencode/scripts/lib/emit.ts",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/README.md",
      "tools/toolu-opencode/contract/capability-matrix.json",
      "docs/opencode.md",
      "docs/portable-core.md",
      "docs/opencode-host-contract.md",
      "tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts",
      "tooling/src/__tests__/opencode-host-contract.test.ts"
    ],
    "input": "Repository plugin sources and the committed contract evidence",
    "check": "bun run check:opencode-surface && bun run test:portable-core && bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts tooling/src/__tests__/opencode-host-contract.test.ts && ! grep -q 'Wire OpenCode to those paths' docs/opencode.md && ! grep -q 'handled by \\[OP-11\\]' tools/toolu-opencode/README.md && ! grep -q 'handled separately by OP-11' tools/toolu-opencode/scripts/lib/emit.ts && grep -q 'Global selection' docs/opencode.md && grep -q 'global selection' docs/portable-core.md && grep -q 'config hook' tools/toolu-opencode/README.md && grep -q 'external_directory' docs/opencode.md && grep -q 'surfaces.npm-clean' docs/opencode.md",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "tooling/src/opencode-host/scenarios-install.ts (one shared session/run/assert helper; export only INSTALL_SCENARIOS): surfaces.npm-clean, surfaces.lifecycle, surfaces.precedence, surfaces.skill-roots (six roots plus the disable-flag, dot-dir and BOM/CRLF parity cases), surfaces.both-routes and surfaces.selection on the pinned host; packTarball gains an optional stage transform for the changed-description tarball; register in opencode-entry-smoke.ts ALL_SCENARIOS (appended); update opencode-entry-smoke.test.ts",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["docs"],
    "paths": [
      "tooling/src/opencode-host/scenarios-install.ts",
      "tooling/src/opencode-host/scenarios-entry.ts",
      "tooling/src/opencode-entry-smoke.ts",
      "tooling/src/__tests__/opencode-entry-smoke.test.ts",
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/**"
    ],
    "input": "Packed @toolu/opencode tarballs (the committed package, plus one whose staged generated/skills/toolu-debug/SKILL.md description differs), isolated pinned opencode-ai@1.18.34 profiles, the scripted loopback provider",
    "check": "bun test --timeout 60000 tooling/src/__tests__/opencode-entry-smoke.test.ts && bun run typecheck && bun run knip && bun run jscpd && bun run smoke:opencode-entry surfaces.npm-clean surfaces.lifecycle surfaces.precedence surfaces.skill-roots surfaces.both-routes surfaces.selection",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate: bun run test, in the host-portable form recorded under Deviations when the known root/merged-/usr failures reproduce on clean origin/main",
    "ac_refs": ["AC-8"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "The whole repository",
    "check": "bun run test:conventions && PATH=\"$(printf %s \"$PATH\" | tr ':' '\\n' | grep -vxE '/bin|/sbin' | paste -sd:)\" bun test --timeout 60000 -t '^(?!.*(refuses the link|unreadable .* file fails|import cost|costs under)).*$' tooling/src packages tools plugins && bun run test:portable-core && bun run test:gate-coverage && bun run test:final-removal && bun run check:plugin-bundles && bun run check:hooks-json && bun run test:workspace && bun run test:pack && bun run test:conformance && bun run test:context-budget && bun run benchmarks --tier deterministic",
    "model": "inherit"
  }
]
```

## Critical files

- **Create:**
  - `tools/toolu-opencode/src/surfaces/{frontmatter,catalog,plan,skill-names,merge,apply}.ts`;
  - `tools/toolu-opencode/src/surfaces/__tests__/{frontmatter,catalog,plan,skill-names,apply}.test.ts`;
  - `tools/toolu-opencode/src/plugin/__tests__/surfaces-hook.test.ts`;
  - `tooling/src/opencode-host/scenarios-install.ts`.
- **Modify:**
  - `tools/toolu-opencode/guardrails.config.json`;
  - `src/inventory/selection.ts`, `src/select/resolve.ts` and `src/host/roots.ts`;
  - `src/plugin/{context,enforcement,hooks}.ts`, plus the tests holding `HostBinding` literals (`plugin/__tests__/{hooks,context,toolu,roots-config}`);
  - `tooling/src/opencode-host/scenarios-entry.ts`, `tooling/src/opencode-entry-smoke.ts` and its test;
  - `tools/toolu-opencode/scripts/lib/emit.ts`, regenerating `generated/`;
  - `docs/opencode.md`, `docs/portable-core.md`, `tools/toolu-opencode/README.md`, `tools/toolu-opencode/contract/capability-matrix.json` and `docs/opencode-host-contract.md`.

## Verification

- **End to end.** The six live `surfaces.*` scenarios pass on the pinned host and cover:
  - discovery through the native skill tool and the agent and command services;
  - a shared-procedure read under default permissions;
  - lifecycle: reselect, update and remove;
  - user precedence, including a permission rule;
  - every skill root and the parity cases;
  - both routes, and the selection sources.

  Their pass lines go in the PR body.
- **Boundaries.** An invalid selection file fails closed, catalog corruption makes toolu not ready, and malformed config never throws.
- **Regression.** The `gate` step covers the Claude Code and Codex suites, surface, bundle and pack drift, and `check:opencode-host`.
- **Docs.** The `docs` step's assertions show that the old manual-wiring and OP-11 wording is gone and that the new sections exist.

## Delivery

1. Make scoped commits per step. `wip:` is not an allowed prefix, so use `feat(opencode)`, `docs(opencode)` or `test(opencode)`.
2. Run `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-02-opencode-install-discover-surfaces.md --verify`, then read `status` for AC coverage.
3. Run `toolu-review:review` on the committed diff. Push-review state must be v2 and cover every changed file, with zero open findings.
4. `bun plugins/toolu/hooks/dist/verdict.js status` must report `overall: ready`.
5. Fetch `origin`, rebase on `origin/main` if it moved, and re-run the affected checks.
6. `git push -u origin feat/345-opencode-install-and-discover-generated`.
7. Find or create the PR to `main`:
   - conventional title `feat(opencode): install and discover generated surfaces with safe ownership`;
   - the body starts with `Closes Falconiere/toolu#345` and `Part of Falconiere/toolu#334`, then gives the summary, the live smoke pass lines, the gate output, and the five decisions the spec asks the PR reviewer to sign off.
8. Verify the PR's number and its head and base branches, then report `pr-open`.
9. Report `babysit`, then run `pr-babysit:babysit`.
10. On success, report `ready` and stop; the orchestrator owns the merge.

## Deviations

- **Gate form, pre-registered.** The `paths-helper-env` plan recorded these failures on a clean `origin/main` under this runner (root, merged `/usr`):
  - five chmod-as-root cases;
  - ten golden "… not on PATH" cases;
  - the load-sensitive "import cost" timings.

  The `gate` check uses the same host-portable form. If `bun run test` passes verbatim here, it is recorded instead. `bench:shell --assert` is load-sensitive and still runs in CI as part of `bun run test`.

## Plan review

Round 1, **Status:** Needs changes.

- 🔴 Fixed: the guardrails folder tree must allow `src/surfaces`. It is now declared in `catalog-plan`, whose check runs `bun run guardrails`.
- 🟡 Fixed:
  - step checks now add typecheck (`selection`, `wiring`) and knip plus jscpd (`apply`, `live`);
  - every `HostBinding` literal test is declared;
  - `prepareEnforcement` gets an injectable `generatedDir`, and the boundary inputs are named;
  - `live` declares `scenarios-entry.ts`, `bundle-plugins.ts` and `plugins/**`;
  - `docs` and regeneration now run before `live`, and `catalog-plan` scopes only the catalog, skills, agents and commands;
  - the gate takes the host-portable form;
  - a Delivery section is added;
  - jscpd is handled with shared helpers and kind-generic code;
  - the docs check asserts the text changes.
- 🔵 Fixed:
  - the `portable-core.md` selection row, `surfaces.*` in the smoke docs, and the host-contract test are added;
  - the host index gets no new re-export;
  - the selection inputs are listed;
  - `plan.test.ts` is added.

Round 2, **Status:** Approved.

- 🔵 Fixed: `wiring` declares only the four `plugin/__tests__` files that hold `HostBinding` literals. The adapter and bootstrap suites run in the gate.
- 🔵 Fixed: the docs check greps for new-only text in `docs/opencode.md`, `docs/portable-core.md` and the README.
- 🔵 Fixed: `catalog-plan`, `apply` and `live` also run `bun run typecheck`.
