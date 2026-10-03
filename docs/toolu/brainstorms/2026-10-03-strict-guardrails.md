# Strict guardrails for the toolu repo — Brainstorm

**Date:** 2026-10-03
**Mode:** Standalone, full

## Capsule

- **Outcome:** Every tracked TypeScript file in the repo is held to one set of failing gates: size ceilings (300 code lines per file, 60 per function), zero duplication, a folder and plugin layout allowlist, a dependency-direction table, cross-package imports through `@toolu/*` exports only, Zod at every untrusted-input boundary, and a behaviour inventory that ties each hook, gate, quality rule and CLI verb to real-data pass and fail scenarios. A tree that no gate covers fails the build.
- **Material defaults / non-goal:** Extend the existing enforcers (oxlint, the guardrails runner, jscpd, knip) rather than add new tools. Turn the gates on at error level now with a committed list of legacy exemptions that can only shrink, then burn the list down per plugin. Relative imports inside a package stay legal (user decision). Non-goals: changing the rules the published plugins enforce for end users, per-module test files, a coverage percentage, an intra-package alias, and rewriting the guardrails config loader in Zod.
- **Repository evidence:** Most requested rules already exist but run on four trees only (`tooling/src`, `packages/toolu-core/src`, `tools/toolu-opencode/src`, `tools/toolu-conformance/src`; 336 modules). The shipped code in `plugins/*/hooks/src` (143 modules), plugin scripts and skill TypeScript (47 files), `tools/toolu-cli/src` (24 modules) and `tools/toolu-opencode/scripts` get typecheck and format only. `tools/toolu-cli/guardrails.config.json` marks five checks `ownedByLinter` while the package has no `.oxlintrc.json`, so those checks run nowhere.
- **Risk:** The type-aware rule count on plugin code is unmeasured beyond size, assertions and `JSON.parse`. Four open worktrees touch `plugins/*`, so exemption lists and refactors will conflict. "Scenario testing" was read as a behaviour inventory; a different reading changes that gate.
- **Handoff:** `/delivery-flow:delivery-flow` with this file, starting at the coverage phase.

## Measured state

| Requested rule | Enforced today | Gap |
|---|---|---|
| File ≤ 300, function ≤ 60 | oxlint in the four gated trees; plugin scripts at 500 / 150; nothing on `hooks/src` | About 19 files and 37 functions over the ceiling in ungated trees (test files included, so an upper bound). Largest: `plugins/pr-babysit/hooks/src/babysit/fixer-dispatch.ts`, 984 lines |
| No duplicated code | jscpd threshold 0 on the four gated trees | 0.37% duplication in ungated trees; 2.58% in test files, which jscpd ignores everywhere |
| Imports use an alias | 598 cross-package imports use `@toolu/*` exports | 66 relative imports reach into another tree: 44 from `tools/toolu-opencode` tests into `tooling/src`, 20 from non-test `tooling/src` into plugin `__tests__` and `tools/*` |
| Folder structure | `folder-tree` allowlist per gated package | No check on the `plugins/<name>/` layout that `AGENTS.md` documents; `toolu-cli` allowlist declared but not run |
| Architecture | `no-restricted-imports` per package, `import/no-cycle` | No rule on plugin code: plugin to plugin, non-test code to `__tests__`, production code to `@toolu/conformance` are all possible |
| Scenario testing | Gate-coverage inventory lists every hook and built-in gate; `colocated-tests` checks placement only | Nothing requires a quality rule, CLI verb or hook entry to have a failing scenario. 147 of 479 modules have a same-named test |
| Zod | Other validators banned; assertions, `any` and `no-unsafe-*` banned in gated trees | 102 non-test files call `JSON.parse`; 60 of them import no Zod. Plugin hook code: 34 `JSON.parse` sites, one file importing Zod, 67 `as` assertions (121 with plugin scripts) |
| No mocks | Convention only | Zero uses today, and no rule keeping it at zero |

## Decisions

1. **Coverage first.** Put every tree under the stack the gated trees already pass: an `.oxlintrc.json` extending the base config for each plugin and for `tools/toolu-cli`, the plugin trees in `.jscpd.json` and `knip.json`, and script directories linted alongside `src`. Add a coverage check that fails when a tracked `.ts` file is outside every oxlint config, jscpd path or knip workspace, and make the guardrails runner exit 3 when a package lists a check as `ownedByLinter` without a lint config that enables it. This closes the cause, not only today's instances.
2. **Ratchet, not big bang.** Legacy violations go into per-file `overrides` in each `.oxlintrc.json` and per-path `ignore` entries for jscpd. A conventions test fails when an exempted file no longer violates its rule, so the list shrinks and never silently grows. Source-level suppression comments stay banned by the existing `lint-suppressions` check, extended to plugins.
3. **Cross-package alias rule.** A local oxlint rule rejects a relative specifier that resolves outside the importing package or plugin root. The 66 existing cases are fixed by exporting what is shared: test harness cases that benchmarks reuse move behind `@toolu/conformance` exports, and `tooling` exposes the modules `tools/toolu-opencode` tests need.
4. **Architecture as data.** One layer table states the allowed directions: core imports no plugin, tool or tooling code; a plugin imports `@toolu/core/*` and never another plugin; non-test code never imports `__tests__` or `@toolu/conformance`. The per-tree `no-restricted-imports` patterns are checked against that table in `test:conventions`, keeping oxlint the single enforcer.
5. **Plugin layout check.** A guardrails check validates each `plugins/<name>/` against the documented layout: required `plugin.json` and `README.md`, allowed top-level entries, a top-level `hooks/src` file is an entry and helpers live in subdirectories, `settings/` only in core.
6. **Behaviour inventory.** Extend the gate-coverage inventory: each hook entry, built-in gate, quality rule under `hooks/src/rules/`, CLI verb and skill CLI names at least one pass scenario and one deny or failure scenario, each resolved to a real test. A missing or dangling scenario fails. A lint rule bans `mock`, `mock.module` and `spyOn` from `bun:test`.
7. **Zod at boundaries.** A local oxlint rule requires the result of `JSON.parse`, `Response.json()` and `Bun.file().json()` to pass straight into a schema's `parse` or `safeParse`, or through one shared helper in `@toolu/core`. The assertion, `any` and `no-unsafe-*` rules extend to plugin code, removing the cast as an escape hatch. Environment reads through a validated helper follow in the burn-down phase.
8. **Where new code lives.** New lint rules go in a local oxlint plugin beside the vendored one, and new runner checks are registered as toolu-local additions recorded in `tooling/conventions/PROVENANCE.md`. The vendored directory stays byte-identical to its upstream pin.

Further rules worth adding in the same pass, all cheap in oxlint: `complexity`, `max-depth` and `max-params` ceilings; `typescript/ban-ts-comment`; no committed `.only` or `.skip` tests; `no-console` in hook code, where stdout is the host protocol.

## Phases

1. **Coverage:** configs, exemption lists, coverage check, the `ownedByLinter` fail-closed fix. Config only; no behaviour change, no version bump.
2. **New rule kinds:** cross-package alias rule with its 66 fixes, layer table, plugin layout check, behaviour inventory, Zod boundary rule, mock ban.
3. **Burn-down:** one plugin per change, largest first (`pr-babysit`, `epic-orchestrator`), each rebuilding its `hooks/dist` bundles.

## Alternatives rejected

- **Fix every violation before enabling the gate:** dozens of refactors to shipped code while four plugin branches are open, and new code stays ungated until the last one lands.
- **Enable one tree at a time:** leaves the remaining trees ungated in the meantime; the ratchet gives strictness on new code everywhere from the first change.
- **dependency-cruiser or eslint-plugin-boundaries for architecture:** a second enforcer and a new dependency for what `no-restricted-imports` plus one local rule already express.
- **Intra-package alias banning `../`:** about 870 import rewrites and alias wiring for plugins that have no `package.json`; declined by the user in favour of the cross-package rule.
- **Coverage percentage floor:** suites spawn real bundles in subprocesses, which in-process coverage does not see.
- **A same-named test per module:** 332 modules lack one, and tests here are scenario suites over real bundles rather than per-module units.
- **Zod for the guardrails config loader:** the loader is held to golden parity with the upstream bash verdicts and exit codes.

Jev preferred extending coverage over new rule kinds first (0.77), the ratchet over fix-all and tree-by-tree (0.71, 0.01, 0.28), the cross-package alias scope (0.83), the behaviour-inventory reading of scenario testing (1.00, with ambiguity rated 0.84), and boundary validation as the meaning of the Zod request (0.86), rating its one-change cost as large (1.82 of 2), which supports the ratchet for that rule as well.

## Open for the spec

- Full type-aware violation count on plugin code under the base config, to size the exemption lists.
- Whether test files take the 300-line ceiling and the duplication gate, or stay exempt as they are in `packages/toolu-core` today.
- Sequencing against the open `plugins/*` worktrees.
