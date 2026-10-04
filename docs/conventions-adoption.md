# Conventions adoption (#213)

**Upstream pin:** [`Falconiere/toolu-conventions@3562c63eb29a05bdb3cbcae5380196043c159759`](https://github.com/Falconiere/toolu-conventions/tree/3562c63eb29a05bdb3cbcae5380196043c159759)  
**Provenance / update procedure:** [`tooling/conventions/PROVENANCE.md`](../tooling/conventions/PROVENANCE.md)  
**Epic:** [#203](https://github.com/Falconiere/toolu/issues/203) · **Issue:** [#213](https://github.com/Falconiere/toolu/issues/213)

This repo is a **library/plugin Bun workspace**, not a Workers/web app. Upstream application scaffolds (Hono, Cloudflare, Vitest, UI/database trees, deploy hooks) are out of scope. Package dirs `packages/toolu-core/` and `tools/toolu-opencode/` join the workspace in [#208](https://github.com/Falconiere/toolu/issues/208) / [#210](https://github.com/Falconiere/toolu/issues/210); this issue gates the existing `tooling/` TypeScript surface.

## Adoption table

| Upstream | Disposition | Destination | Rationale / test |
|----------|-------------|-------------|------------------|
| `guardrails/run.sh` + `lib/` + `checks/` | port | `tooling/src/guardrails/` | TypeScript port on Bun ([#277](https://github.com/Falconiere/toolu/issues/277)); same checks, flags and exit codes, proven against the bash verdicts on the upstream fixtures; `bun run guardrails` |
| `guardrails/patterns/` + schemas | retain | `tooling/conventions/guardrails/` | Data read by the TypeScript runner and by editors |
| `guardrails/oxlint-plugin/` | retain | `tooling/conventions/guardrails/oxlint-plugin/` | Custom oxlint rules; `bun run lint:ts` |
| `lint/base.oxlintrc.json` | adapt | `tooling/conventions/lint/base.oxlintrc.json` + root `.oxlintrc.json` | Fix plugin path; drop Workers-only rules; type-aware unsafe-value rules kept |
| backend-ts `tsconfig.json` strict flags | adapt | `tsconfig.json` | Bun/`@types/bun`; no Vitest/Workers types; same strict set |
| backend-ts `.oxfmtrc.json` | retain | `.oxfmtrc.json` | `bun run format:check` |
| backend-ts `knip.json` | adapt | `knip.json` | Entries = tooling CLIs under `tooling/src/` |
| backend-ts `.jscpd.json` | adapt | `.jscpd.json` | Scan `tooling/src` |
| backend-ts `guardrails.config.json` | adapt | `tooling/guardrails.config.json` | Flat `srcRoot: src`; no wrangler; fn max **60** |
| `guardrails.workspace.json` | adapt | `guardrails.workspace.json` | `packages: ["tooling"]` until #208 |
| CORE.md Zod / no-any / no-assertions | retain | oxlint + docs | Fixtures in `tooling/fixtures/conventions/` |
| CORE.md size ceilings 300 / 50 | adapt | oxlint 300 / **60** | Align Bun gate with `plugins/ts-quality` defaults (file 300, fn 60); upstream 50 not adopted |
| Workers `barrelExempt` `src/index.ts` | n/a | — | No Worker default-export entry |
| Vitest / wrangler templates | n/a | — | Bun test only; no Workers |
| Hono / UI / database STRUCTURE | n/a | — | Library/plugin workspace |
| Lefthook templates | n/a (document only) | this doc | **Do not** install Lefthook as a second Claude hook pipeline; existing registry/dispatcher stays authoritative. Developers may adopt Lefthook locally later without overwriting Claude hooks — tracked as docs-only here; CI wiring is #208. |
| Live upstream fetch in CI/hooks | n/a | — | Pin in-tree only (PROVENANCE) |

## Threshold ownership

| Rule | Bun / local SoT | Claude `ts-quality` hook | Notes |
|------|-----------------|--------------------------|-------|
| File ≤ **300** code lines | oxlint `max-lines` | `25-size-file.sh` default 300 | Aligned; every TypeScript tree, plugin hooks and scripts included |
| Function ≤ **60** code lines | oxlint `max-lines-per-function` | `30-size-fn.sh` default 60 | Aligned with hook; upstream template used 50; every TypeScript tree |
| No `any` / assertions / non-null | oxlint type-aware | concerns | Repo TS CI/local SoT once #208 wires CI |
| Colocated `__tests__` | guardrails + oxlint plugin | `20-tests.sh` | Tests are `__tests__/*.test.ts` under `src/`, tooling suites included (`tooling/src/__tests__`, see [`testing.md`](testing.md)) |
| Banned validators | guardrails `bannedDeps` | — | Zod only (`package.json` dependency) |
| Dead code / duplication | knip / jscpd | — | Every TypeScript tree; tests are not scanned for duplication |

`ownedByLinter` in a package's `guardrails.config.json` must name only check ids that oxlint (or its plugin) actually enforces. `bun run check:gate-reach` verifies it for every package in `guardrails.workspace.json`: `folder-tree`, `colocated-tests` and `no-barrels` map to the house rules of the same name, `filename-case` to `unicorn/filename-case`, each at error level in that package's `.oxlintrc.json` (resolved through `extends`). `patterns` maps to no rule, because toolu adopted none of the upstream pattern rules. A package with no lint config, or any other id, fails.

## Gate reach and legacy exemptions

Every tracked `.ts` and `.tsx` file must be reached by five gates: typecheck, format, oxlint, jscpd and knip. `bun run check:gate-reach` reads each gate's own config (the root `tsconfig.json`, the `format:check` script, every `.oxlintrc.json` with its lint targets, `.jscpd.json` and `knip.json`) and fails on a file a gate does not reach. `tooling/gate-reach.json` holds the two ways out:

- `exclude`: generated, vendored and fixture trees that are not source.
- `allowances`: a declared gap for one tool and one glob, with a reason. An allowance that no longer covers an unreached file fails as stale.

Adding a TypeScript tree means adding it to each gate's config, or declaring why not.

**Lint targets.** A directory holding `.oxlintrc.json` lints its `src/`, `scripts/` and `contract/`. `plugins/.oxlintrc.json` is a collection config: it lints `hooks/src/`, `scripts/` and `skills/` of every plugin under the base rules, so no lint config ships inside a plugin.

**Legacy exemptions.** Code that predates a gate is exempted per file, by exact path, in the owning tool's own config:

| Tool | Exemption |
|------|-----------|
| oxlint | an `overrides` block naming the file, with each violated rule set to `"off"` |
| jscpd | an `ignore` entry `**/<repo path>` |
| knip | a workspace `ignore` entry naming the file |

An entry with glob characters is a structural rule, not an exemption, and is never checked for staleness. Three shapes are refused as misconfiguration (exit 3) because they cannot be read either way: a bare path in the jscpd `ignore` list (jscpd matches absolute paths and would not honour it), an off-only oxlint override that mixes exact paths with globs, and a negated (`!`) pattern in any config the reach check reads.

`bun run check:legacy-exemptions` re-runs each tool with these lifted and fails when an exempted file is gone or no longer has the finding. Fix the code, then delete the entry. Do not add one for new code: nothing compares the lists with an earlier revision, so a new entry is caught only in review, and an `.oxlintrc.json` edit prompts through the protected-files gate.

## Canonical Bun scripts

| Script | Gate |
|--------|------|
| `bun run format:check` | oxfmt |
| `bun run lint:ts` | type-aware oxlint over every `.oxlintrc.json` directory and all of its lint targets (workspace packages and the plugin collection), via `tooling/src/lint-ts.ts` |
| `bun run typecheck` | `tsc --noEmit` |
| `bun run guardrails` | `tooling/src/guardrails/run.ts` (TypeScript port of the upstream `run.sh`) |
| `bun run knip` | dead code |
| `bun run jscpd` | duplication |
| `bun run check:gate-reach` | every TypeScript file reached by every gate, and `ownedByLinter` backed by a lint rule |
| `bun run check:legacy-exemptions` | no per-file exemption outlives its finding |
| `bun run test:unit` | `bun test` in `tooling/`, `packages/`, `tools/`, and `plugins/` |
| `bun run test:workspace` | Bun 1.4.x pin + export smoke |
| `bun run test:conventions` | format/lint/tsc/guardrails/knip/jscpd/gate reach/legacy exemptions + Bun convention tests |
| `bun run test:ts` | complete Bun gate: conventions, unit, portable-core, gate coverage, bundle/launcher drift, workspace/package, conformance, context budget, deterministic benchmarks, and shell-analysis latency budget (CI `bun run test` job) |
| `bun run test` | delegates to `test:ts` |

`bun install --frozen-lockfile` is required before these scripts in CI and locally after dependency changes. Missing required workspace config or tooling is an error, not a successful skip.

## Local TS CI

```bash
bun install --frozen-lockfile
bun run test
```

CI job `bun run test` (`gate`) in `.github/workflows/tests.yml` runs the same gate on every non-release-only PR/push. The required `typescript` status passes only when that job and the OpenCode acceptance jobs pass. The retired `shellcheck` and `bats (plugins)` jobs no longer report, so branch protection must require `typescript` in their place. The repo-admin branch-protection setting is tracked separately from this change.

## Related contracts

- Package / Zod / Bun boundaries: [`docs/portable-core.md`](portable-core.md)
- Behavior inventory: [`docs/gate-coverage-matrix.md`](gate-coverage-matrix.md)
