# Strict guardrails, phase 1: gate reach — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Claude   **Topic:** Every tracked TypeScript file is reached by every gate, or sits on a visible exemption that fails once it is no longer needed

## Problem

The repo's quality rules (base oxlint rules, 300-line files, 60-line functions, zero duplication, no dead exports) run on four trees only. The shipped plugin code and three other trees get typecheck and format, and one tree gets neither:

- `plugins/*/hooks/src` (143 modules), plugin `scripts/` and `skills/` TypeScript and `tools/toolu-cli/src` (24 modules) have no base lint rules, no jscpd and no knip.
- `tools/toolu-opencode/scripts` (15 files) is outside every tsconfig, `format:check`, lint, jscpd and knip. Typechecking it yields 8 errors. `tools/toolu-opencode/contract` is typechecked and formatted but not linted.
- `tools/toolu-cli/guardrails.config.json` marks five checks `ownedByLinter`, but the package has no `.oxlintrc.json`, so the runner skips them and no linter runs them.

Nothing fails when a tree is outside a gate, so the gap grows silently. Brainstorm: `docs/toolu/brainstorms/2026-10-03-strict-guardrails.md`.

## Non-Goals

1. Fixing legacy lint, duplication or dead-export findings in plugin or tool source. They are exempted per file here and burned down in phase 3.
2. New rule kinds (cross-package alias rule, layer table, plugin layout check, behaviour inventory, Zod boundary rule, mock ban). Phase 2.
3. Adding plugins to the guardrails workspace (`folder-tree`, `lint-suppressions`). Phase 2, with the plugin layout check.
4. Linting or duplicate-scanning test files in trees that do not already do so. Tests stay where each package has them today; the allowance is declared, not silent.
5. Changing the ported guardrails runner or the vendored `tooling/conventions/` tree.
6. Changing any shipped hook bundle (`hooks/dist`) or plugin behaviour.
7. Preventing a hand-added exemption. No check compares the lists with an earlier revision; a new entry is visible in review, and a lint-config edit already prompts through the protected-files gate.

## Architecture

Three additions, all toolu-local and outside the vendored runner.

**1. Reach.** `lint-ts.ts` lints every target a config directory has instead of the first one: `src`, `scripts` and `contract` for a package, and `*/hooks/src`, `*/scripts`, `*/skills` for a collection directory that has no `src`. One new `plugins/.oxlintrc.json` extends the base config with the 300 / 60 ceilings for all plugin trees. It replaces the three per-plugin configs (500 / 150, no base rules), so each file has one enforcer and no lint config ships inside a plugin; the generated OpenCode surface is regenerated because it mirrors one of them. `tools/toolu-cli` gains a config modelled on `tools/toolu-conformance`. `.jscpd.json`, `knip.json`, the root `tsconfig.json` and `format:check` are extended to the same trees. The 8 type errors and one unformatted file in `tools/toolu-opencode/scripts` are fixed, because typecheck and format have no per-file exemption (Jev: fix now 0.82).

**2. Legacy exemptions.** An exemption is an exact repo path (no glob characters) in the owning tool's native config: an oxlint `overrides` block whose rules are all `"off"`, a `.jscpd.json` `ignore` entry, or a `knip.json` workspace `ignore` entry (Jev: per-file knip exemption over fixing 44 exports now, 0.81). `check-legacy-exemptions.ts` re-runs each tool with the exemptions lifted and fails when an exempted file no longer has the finding it is exempted for. An exemption therefore cannot outlive its cause. Trade-off: oxlint stays the single enforcer and editors agree with CI, at the cost of a second tool run in the gate.

**3. Reach check.** `check-gate-reach.ts` lists tracked `.ts` and `.tsx` files and asks, per tool, whether the tool's own config reaches each one. An unreached file fails unless `tooling/gate-reach.json` declares an allowance for that tool, and an allowance that no longer covers any unreached file fails as stale. The same check verifies that every `ownedByLinter` id of every guardrails package maps to a rule enabled at error level in that package's lint config. This lives here rather than in the runner because the runner is held to 229 golden parity cases whose fixture packages have no lint config (Jev: local check 0.97).

Reused: `lint-ts.ts` config and target discovery (shared with the reach check so lint and reach cannot disagree), `envOr`, `Bun.Glob`, Zod for every config the checks read, `@toolu/conformance/harness/sandbox` and `spawn` for tests. New code lives in `tooling/src/gate-reach/` with two entry scripts in `tooling/src/`.

## Interfaces / Schema

`tooling/gate-reach.json` (strict; unknown keys rejected):

```json
{
  "version": 1,
  "exclude": ["**/fixtures/**", "**/hooks/dist/**"],
  "allowances": [
    { "tool": "oxlint", "glob": "**/__tests__/**", "why": "tests are linted only where a package config opts in" }
  ]
}
```

`tool` is one of `typecheck`, `format`, `oxlint`, `jscpd`, `knip`. `exclude` removes generated, vendored and fixture trees from the file universe.

Reach per tool, from that tool's committed config:

| Tool | Reached when |
|---|---|
| `typecheck` | matches a root `tsconfig.json` `include` glob and no `exclude` glob |
| `format` | matches a path argument of the `format:check` script (the arguments after `oxfmt --check`; a directory argument reaches everything below it) |
| `oxlint` | under a lint target of a directory holding `.oxlintrc.json`, and not matched by its `ignorePatterns` |
| `jscpd` | under a `.jscpd.json` `path` and not matched by a glob `ignore` entry |
| `knip` | matches a `project` glob of its `knip.json` workspace and no glob `ignore` entry |

Exact-path `ignore` entries are legacy exemptions and do not remove reach. jscpd matches `ignore` against absolute paths, so its exact-path entries carry one leading `**/` (amended during execution).

`ownedByLinter` backing: `folder-tree` → `house/folder-tree`, `colocated-tests` → `house/colocated-tests`, `no-barrels` → `house/no-barrels`, `filename-case` → `unicorn/filename-case`, `patterns` → no rule (toolu adopted none of the upstream pattern rules). Rules are resolved through `extends`. Any other id fails.

Scripts: `check:gate-reach` and `check:legacy-exemptions`, both run by `test:conventions`. `GATE_REACH_ROOT` and `LEGACY_EXEMPTIONS_ROOT` point them at another tree, as `LINT_TS_ROOT` does. Exit codes follow the guardrails runner: `0` clean, `1` findings, `3` misconfigured.

Output, one line per finding:

```
gate-reach: plugins/x/tools/stray.ts: not reached by oxlint
gate-reach: stale allowance: jscpd tools/old/** covers no unreached file
gate-reach: tools/toolu-cli: ownedByLinter "no-barrels" has no rule at error level in tools/toolu-cli/.oxlintrc.json
legacy-exemptions: plugins/.oxlintrc.json: jev/hooks/src/jev.ts no longer violates max-lines; remove the exemption
legacy-exemptions: .jscpd.json: plugins/jev/hooks/src/session-start.ts has no clone; remove the exemption
```

## Failure modes and edge cases

- **Config missing, not JSON, or failing its schema** (`gate-reach.json`, a tool config, a package config, a `format:check` script without `oxfmt --check`): exit 3 naming the file and key. Never treated as "no rules".
- **Not a git work tree, or zero tracked TypeScript files after `exclude`:** exit 3; a gate that governs nothing must not pass.
- **Tool crash or unparseable report** (oxlint, jscpd, knip exit outside 0 and 1, or invalid JSON): exit 3 with the tool's stderr tail. A failed run is never read as "no findings".
- **Exempted file deleted or renamed:** reported as stale, exit 1.
- **Exempted file violates a rule it is not exempted for, or a non-exempt file violates any rule:** `lint:ts` fails as it does today; exemptions are per file and per rule.
- **Lint config directory with no lint target:** `lint:ts` exits 1 naming the directory (existing behaviour, kept).
- **Allowance and reach both true:** the allowance is stale and fails, so allowances shrink as trees are brought in.
- **Temporary lifted oxlint config:** written beside the real config so relative `extends`, `ignorePatterns` and `overrides` globs keep their meaning, named with the process id, and removed in a `finally` block; a leftover from a killed run is not named `.oxlintrc.json` and is ignored by lint discovery.
- **Open branches:** a branch that adds a violation in a newly gated tree fails after rebasing onto this change. That is the intended effect, noted in the PR description.

## Acceptance criteria

- **AC-1:** Given the repo, `bun run lint:ts` lints `plugins/*/hooks/src`, plugin `scripts` and `skills` TypeScript, `tools/toolu-cli/src`, and `tools/toolu-opencode/scripts` and `contract` under the base rules with 300 / 60 ceilings and exits 0. Given a sandbox collection config that enables `typescript/no-explicit-any` and a plugin file containing an explicit `any`, it exits 1 naming `no-explicit-any`; given a config directory with both `src` and `scripts`, a violation in either fails; given one with no target, it exits 1 naming the directory.
- **AC-2:** Given the repo, `bun run check:gate-reach` exits 0. Given a sandbox repo with a tracked `.ts` file outside one tool's reach, it exits 1 with a line naming the file and that tool, for each of the five tools; with a matching allowance it exits 0; with an allowance that covers no unreached file it exits 1 naming the stale allowance; with an unknown key in `gate-reach.json`, or outside a git work tree, it exits 3.
- **AC-3:** Given a sandbox package whose `guardrails.config.json` lists `ownedByLinter: ["no-barrels"]`, `check:gate-reach` exits 1 naming the package and id when the package has no `.oxlintrc.json` or the rule is not at error level, exits 1 on an id outside the backing table, and exits 0 when the rule is enabled through `extends`. On the repo, `tools/toolu-cli` passes.
- **AC-4:** Given the repo, `bun run check:legacy-exemptions` exits 0. Given a sandbox where an oxlint-exempted file no longer violates the exempted rule, a jscpd-exempted file has no clone, a knip-exempted file has no finding, or an exempted file is missing, it exits 1 naming the config, the file and the rule or tool; where each exempted file still has its finding, it exits 0.
- **AC-5:** Given the repo, `bun run jscpd`, `bun run knip`, `bun run typecheck` and `bun run format:check` exit 0, and `tooling/gate-reach.json` holds no allowance for a non-test file under `plugins/`, `tools/toolu-cli/src` or `tools/toolu-opencode/scripts`. Given a sandbox holding the repo's `.jscpd.json` and two plugins with the same 15-line function under `hooks/src`, jscpd exits non-zero naming both files.
- **AC-6:** Given the repo, `bun run test:conventions` runs both new checks, and `bun run test` passes with no change to any `plugins/*/hooks/dist` bundle.
- **AC-7:** `docs/conventions-adoption.md` and `AGENTS.md` describe the reach check, allowances, legacy exemptions and the two scripts; the conventions test fails if either script name is absent from the adoption doc.

## Acceptance evidence

| AC | Real input | Expected result | Boundary / failure case | Check |
|---|---|---|---|---|
| AC-1 | The repo; sandbox trees built with `createSandbox` and real oxlint | Exit 0 on the repo; exit 1 naming the rule in the sandbox | Config directory with no target; violation only in the second target | `bun run lint:ts && bun test tooling/src/__tests__/lint-ts.test.ts` |
| AC-2 | The repo; sandbox git repos with real tool configs, one unreached file per tool | Exit 0 on the repo; 1, 0, 1, 3, 3 in the sandbox cases | Stale allowance; unknown key; non-git directory | `bun run check:gate-reach && bun test tooling/src/gate-reach/__tests__/reach.test.ts` |
| AC-3 | Sandbox guardrails workspace with real config files | Exit 1, 1, 1, then 0 | Rule only present through `extends`; id outside the table | `bun test tooling/src/gate-reach/__tests__/owned-rules.test.ts` |
| AC-4 | The repo; sandbox trees run through real oxlint, jscpd and knip | Exit 0 on the repo; exit 1 naming each stale entry; exit 0 when findings remain | Missing exempted file | `bun run check:legacy-exemptions && bun test tooling/src/gate-reach/__tests__/legacy.test.ts` |
| AC-5 | The repo; a sandbox with the repo's `.jscpd.json` and a duplicated function in two plugins | Four tools exit 0; jscpd exits non-zero in the sandbox | Duplicate across two plugin trees | `bun run jscpd && bun run knip && bun run typecheck && bun run format:check && bun test tooling/src/gate-reach/__tests__/repo-reach.test.ts` |
| AC-6 | The repo | Gate passes; `git diff --stat main -- 'plugins/*/hooks/dist'` is empty | Bundle drift check inside the gate | `env -u npm_config_store_dir TMPDIR=/private/tmp bun run test` |
| AC-7 | The two docs | Script names and terms present | Conventions test asserts the adoption doc | `bun test tooling/src/__tests__/conventions-guardrails.test.ts` |

## Documentation impact

- `docs/conventions-adoption.md`: threshold ownership now covers plugin and CLI trees; a new section on reach, allowances and legacy exemptions; the two scripts in the script table.
- `AGENTS.md`: key-file rows for the two checks and `tooling/gate-reach.json`; the `tests.yml` row mentions gate reach; the contributing note that a new TypeScript tree must be reached or allowed.
- No plugin README, `SKILL.md` or release note changes: no shipped behaviour changes.

## Open Questions

- Whether test files take the lint and duplication gates everywhere. Owner: Falconiere. Non-blocking: declared as allowances here, decided in phase 2.
- Order of the phase 3 burn-down against the open `plugins/*` branches. Owner: Falconiere. Non-blocking.

## Spec review

**Status:** Approved after three fixes.

- Topic: 🔴 blocker: "an exemption that can only shrink" overstated the mechanism, which removes stale entries but cannot stop a hand-added one (Jev 0.15 that the claim held). Fixed: topic reworded and Non-Goal 7 states the limit.
- AC-2: 🟡 should-fix: the evidence named a non-git case the criterion did not state (Jev alignment 0.46). Fixed: the criterion now states exit 3 outside a work tree and one unreached file per tool.
- AC-5: 🟡 should-fix: exit 0 from four tools did not show the new trees are scanned (Jev: needs its own case, 0.83). Fixed: the criterion now requires no allowance for those trees and a duplicate across two plugin trees that the repo's own jscpd config rejects.
- Architecture: 🔵 consider: `tools/toolu-opencode/contract` was typechecked but not linted. Fixed: `contract` is a package lint target.
