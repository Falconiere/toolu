# Strict guardrails, phase 1: gate reach — Plan

**Date:** 2026-10-03   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-03-strict-guardrails-reach-design.md   **Topic:** Bring every TypeScript tree under the existing gates, with stale-proof exemptions and a reach check.

## Evidence and approach

Inspected: `tooling/src/lint-ts.ts` lints the first of `src` or `scripts` per `.oxlintrc.json` directory; `tooling/src/__tests__/lint-ts.test.ts` runs it in sandboxes with real oxlint. `.jscpd.json` and `knip.json` list four trees. The root `tsconfig.json` and `format:check` omit `tools/toolu-opencode/scripts`. Three plugin lint configs carry 500 / 150 ceilings without the base rules. `plugins/toolu/settings/protected-files.txt` makes every `.oxlintrc.json` write prompt the user, so each lint config is written once, complete.

Measured on `main` at `33bd8798` with the base config: 42 plugin hook modules, 27 plugin script files, 7 CLI modules, 10 OpenCode script files and 4 contract probes have findings; jscpd finds 4 clones across 7 files; knip finds 44 unused exports across about 30 files; `tools/toolu-opencode/scripts` has 8 type errors in two files and one unformatted file.

Approach: no plugin or tool source changes except those 8 type fixes and one format fix. Exemptions are seeded from each tool's JSON report by a throwaway helper outside the repo, never by hand. The reach and legacy checks are new modules under `tooling/src/gate-reach/` that parse every config with Zod and share lint discovery with `lint-ts.ts`. The ported guardrails runner and `tooling/conventions/` are untouched.

## Workstream summary

Lint discovery → lint reach and typecheck/format reach → duplication and dead-code reach → reach check → legacy-exemption check → gate wiring and docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "lint-targets",
    "title": "lint-ts lints every target of a config directory (src, scripts, contract; or */hooks/src, */scripts, */skills for a collection) and exports its discovery",
    "ac_refs": ["AC-1"],
    "paths": ["tooling/src/lint-ts.ts", "tooling/src/__tests__/lint-ts.test.ts"],
    "input": "Sandbox trees with real oxlint: a collection config enabling typescript/no-explicit-any over two plugin dirs with an explicit any in one; a package config with src and scripts where only scripts violates; a config directory with no target; node_modules and the OpenCode plugin mirror present",
    "check": "bun test --timeout 60000 tooling/src/__tests__/lint-ts.test.ts",
    "model": "inherit"
  },
  {
    "id": "lint-reach",
    "title": "One plugins/.oxlintrc.json and a tools/toolu-cli config on the base rules with 300/60, per-file legacy overrides seeded from oxlint reports; OpenCode scripts and contract linted, typechecked and formatted; three per-plugin configs removed and the OpenCode surface regenerated",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["lint-targets"],
    "paths": ["**"],
    "input": "The repo: 143 plugin hook modules, plugin scripts and skills, tools/toolu-cli/src, tools/toolu-opencode/scripts and contract; the 8 type errors in generate-surface.ts and its test",
    "check": "bun run lint:ts && bun run typecheck && bun run format:check && bun run check:opencode-surface && bun run guardrails",
    "model": "inherit"
  },
  {
    "id": "dup-dead-reach",
    "title": "jscpd and knip scan plugin trees, tools/toolu-cli/src and tools/toolu-opencode scripts and contract, with exact-path legacy ignores seeded from their reports",
    "ac_refs": ["AC-5"],
    "depends_on": ["lint-reach"],
    "paths": ["**"],
    "input": "The repo: the 4 cross-plugin clones and 44 unused exports measured on main",
    "check": "bun run jscpd && bun run knip",
    "model": "inherit"
  },
  {
    "id": "reach-check",
    "title": "check-gate-reach: per-tool reach from each tool's own config, allowances in tooling/gate-reach.json with stale detection, and ownedByLinter backing",
    "ac_refs": ["AC-2", "AC-3", "AC-5"],
    "depends_on": ["dup-dead-reach"],
    "paths": ["**"],
    "input": "The repo; sandbox git repos with real tsconfig, package.json, .oxlintrc.json, .jscpd.json and knip.json and one unreached tracked file per tool; a matching allowance; a stale allowance; gate-reach.json with an unknown key; a non-git directory; a guardrails workspace whose package owns no-barrels with no lint config, with the rule off, with an unknown id, and with the rule enabled through extends; the repo's .jscpd.json over a function duplicated in two sandbox plugins; the repo's tooling/gate-reach.json, asserted to hold no allowance matching a non-test file under plugins/, tools/toolu-cli/src or tools/toolu-opencode/scripts",
    "check": "bun run check:gate-reach && bun test --timeout 60000 tooling/src/gate-reach/__tests__/reach.test.ts tooling/src/gate-reach/__tests__/owned-rules.test.ts tooling/src/gate-reach/__tests__/repo-reach.test.ts",
    "model": "inherit"
  },
  {
    "id": "legacy-check",
    "title": "check-legacy-exemptions: re-run oxlint, jscpd and knip with exact-path exemptions lifted and fail on any exemption whose finding is gone",
    "ac_refs": ["AC-4"],
    "depends_on": ["reach-check"],
    "paths": ["**"],
    "input": "The repo's committed exemptions; sandbox trees run through real oxlint, jscpd and knip where an exempted file still violates, no longer violates, has no clone, has no dead export, or is missing; a tool report that is not JSON",
    "check": "bun run check:legacy-exemptions && bun test --timeout 120000 tooling/src/gate-reach/__tests__/legacy.test.ts",
    "model": "inherit"
  },
  {
    "id": "wire-docs",
    "title": "test:conventions runs both checks; conventions-adoption.md and AGENTS.md document reach, allowances, legacy exemptions and the scripts; the conventions test asserts the script names",
    "ac_refs": ["AC-6", "AC-7"],
    "depends_on": ["legacy-check"],
    "paths": ["**"],
    "input": "package.json scripts, docs/conventions-adoption.md, AGENTS.md; a conventions test reading the real files that fails when test:conventions omits check:gate-reach or check:legacy-exemptions, or when the adoption doc omits either script name",
    "check": "bun test --timeout 60000 tooling/src/__tests__/conventions-guardrails.test.ts && bun run test:conventions",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate on the final branch with no hooks/dist change",
    "ac_refs": ["AC-6"],
    "depends_on": ["wire-docs"],
    "paths": ["**"],
    "input": "Whole branch diff; git diff --stat main -- 'plugins/*/hooks/dist' must be empty",
    "check": "test -z \"$(git diff --name-only main -- 'plugins/*/hooks/dist')\" && bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Create: `plugins/.oxlintrc.json`, `tools/toolu-cli/.oxlintrc.json`, `tooling/gate-reach.json`, `tooling/src/check-gate-reach.ts`, `tooling/src/check-legacy-exemptions.ts`, and under `tooling/src/gate-reach/`: `reach-schema.ts`, `reach-files.ts`, `reach-tools.ts`, `reach-run.ts`, `owned-rules.ts`, `legacy-schema.ts`, `legacy-tools.ts`, `legacy-run.ts`, plus `__tests__/reach.test.ts`, `owned-rules.test.ts`, `repo-reach.test.ts`, `legacy.test.ts` and shared sandbox helpers.

Modify: `tooling/src/lint-ts.ts`, `tooling/src/__tests__/lint-ts.test.ts`, `tooling/src/__tests__/conventions-guardrails.test.ts`, `tooling/guardrails.config.json` (allow the `gate-reach` directory), `tools/toolu-opencode/.oxlintrc.json`, `tools/toolu-opencode/tsconfig.json`, `tsconfig.json`, `.jscpd.json`, `knip.json`, `package.json`, `tools/toolu-opencode/scripts/generate-surface.ts`, `tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts`, one unformatted file under `tools/toolu-opencode/scripts/lib/`, `docs/conventions-adoption.md`, `AGENTS.md`.

Delete: `plugins/toolu/.oxlintrc.json`, `plugins/epic-orchestrator/.oxlintrc.json`, `plugins/toolu/skills/setup/.oxlintrc.json`, and the generated mirror `tools/toolu-opencode/generated/skills/toolu-setup/.oxlintrc.json` through `bun run generate:opencode-surface`.

Module split keeps each file under 300 code lines and each function under 60.

## Verification

Each behaviour step is test-first: the sandbox test is written and seen red before its module exists. Config steps are proven by the real tools exiting 0 on the repo, then by the reach and legacy checks, which fail if a tree is dropped or an exemption goes stale. Failure propagation is covered by tests for exit 3 on a bad config, a non-git directory and an unparseable tool report.

End to end: `env -u npm_config_store_dir TMPDIR=/private/tmp bun run test` on the final branch, and an empty `git diff --name-only main -- 'plugins/*/hooks/dist'`.

Delivery: fetch and rebase on `origin/main` before the final gate and again before pushing if it moved, re-running every affected step. Scoped commit including the delivery documents; `plan-ledger.js run <plan> --verify`; `toolu-review:review` with version 2 state covering every changed file; `verdict.js status` reporting `overall: ready`; push `strict-quardrails`; open the pull request against `main` and verify its head and base; invoke `pr-babysit:babysit`. Prerequisites: `gh api user` succeeds, the branch is not `main`, and the `pr-babysit` skill is installed.

Each `check:*` script is added to `package.json` in the step that creates its entry file; `wire-docs` adds both to `test:conventions`.

## Plan review

**Status:** Approved after two fixes. The ledger parses, every step has a runnable check, all seven spec ACs are referenced with no dangling id, and dependencies run config reach before the checks that read it.

- reach-check: 🟡 should-fix: no declared input proved the "no allowance for the newly reached trees" half of AC-5 (Jev 0.23 that the steps covered it). Fixed: `repo-reach.test.ts` asserts it against the committed `tooling/gate-reach.json`.
- wire-docs: 🟡 should-fix: running `test:conventions` would still pass if a check were later removed from it (Jev 0.48). Fixed: the conventions test asserts both names in the script.

## Deviations

- **jscpd exemption form.** jscpd matches `ignore` against absolute paths, so a bare repo path is not honoured. A jscpd legacy exemption is written `**/<repo path>`; "exact path" means no glob characters after one leading `**/`. Lifting one clone exposed a transitive one, so four files are exempted, not three.
- **Reach check found two gaps the measurement missed.** `format:check` listed three globs under `plugins/epic-orchestrator/scripts` and skipped the new `launch/` and `watch/` directories; it now takes the directory. A generated `hooks/dist/*.d.ts` is tracked, so `**/hooks/dist/**` is excluded from the file universe.
- **Allowances are per tree for oxlint.** `packages/toolu-core/src/**/__tests__/**` and `plugins/**/__tests__/**` rather than one `**/__tests__/**`, so a package that lints its tests today cannot silently stop.
- **`tools/toolu-cli` tests are linted** with per-file legacy overrides, following the `tools/toolu-conformance` config it is modelled on; ignoring them would leave the `colocated-tests` rule the package hands to the linter with nothing to look at.
- **Module layout.** Schemas for both checks share `reach-schema.ts` (no `legacy-schema.ts`), and the two entry scripts share `gate-main.ts`, which keeps their exit contract in one place and out of jscpd's way.
- **A file in `plugins/` broke one test helper.** The first full gate failed 7 `check-hooks-json` tests: the helper that copies the repo treated every entry of `plugins/` as a plugin directory, and `plugins/.oxlintrc.json` is a file. The helper now copies directories only. Every production walker of `plugins/` (`check-hooks-json.ts`, `build-plugins.ts`, `inventory/scan.ts`, `bundle-plugins.ts`, `manifests.ts`, `codex-smoke.ts`) already skipped non-directories.
- **Generated surface.** `docs/conventions-adoption.md` is mirrored into the OpenCode surface, so the doc edit required `bun run generate:opencode-surface` as well; the same gate run failed the drift test and the conformance drift suite until it was regenerated.
- **Rebased twice** during execution, onto `33bd8798` and `3132f1e5`; exemptions were seeded after the first and the full gate ran after the second.
- **Independent review, eleven findings, all fixed in a second commit.** oxlint `ignorePatterns` with a trailing or leading slash were not recognised, so a file oxlint skipped counted as reached. Negated patterns are now refused eagerly per config (a lazy match let an earlier glob hide one). The `**/` prefix is a jscpd-only exemption form, and a bare jscpd path is fatal. An off-only override with no rules is not an exemption, and one mixing exact paths with globs is fatal. Any failure to finish exits 3 instead of an uncaught exit 1. Lifted configs are named `*.tmp.*`, which the repo already ignores. Tests cover each, plus the `extends` cycle guard and every severity form, and assertions compare the full stderr line.
- **OpenCode lint config** had a trailing comma; it is strict JSON now because both checks parse every config strictly and fail closed.
