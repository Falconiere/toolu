# CI path-scoped jobs — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-ci-path-scoped-jobs-design.md   **Topic:** #458: changes job, job-level gating, always-reporting aggregates, workflow check

## Evidence and approach

- **Inspected:**
  - `.github/workflows/tests.yml` and `toolu-review.yml`, which today use workflow-level `paths-ignore`;
  - `tooling/src/opencode-acceptance/__tests__/workflow.test.ts`, which parses `tests.yml` with `Bun.YAML` and zod, and asserts that `gate` and `opencode` have no `if`/`needs`;
  - `tooling/src/__tests__/workspace-skeleton.test.ts`, which asserts the `paths-ignore` list and `needs: [gate, opencode]` and must change;
  - release PR #390's file list, which includes `tools/toolu-cli/npm/package.json` and version-only lines;
  - `release-please-config.json`'s `extra-files`;
  - branch protection, which requires `typescript`, `review`, `gitleaks` and `Opengrep OSS` (memory cba883c9).
- **Measured:**
  - the `Bun.build` metafile closure of `tooling/src/opencode-*.ts` plus `tools/toolu-opencode/**/*.live.test.ts`;
  - the candidate docs checks, about 10 s and all green.
- **Reused:**
  - `@toolu/conformance/harness/spawn` (`run`) and `harness/sandbox` (`createSandbox`) for real subprocesses and git repos;
  - `zod` and `Bun.Glob` / `Bun.YAML`;
  - the `ROOT` resolution idiom from the tooling tests.
- **Approach:** as in the spec. Logic lives in `tooling/src/ci-paths/`. Three thin entries sit beside the other tooling entries: `ci-changes.ts`, `ci-aggregate.ts` and `check-ci-paths.ts`. Tests use real git repos, real event JSON, real `GITHUB_OUTPUT` files and the real workflow YAML.

## Workstream summary

data + classifier + `changes` entry → aggregate → check → workflows and their existing assertions → OpenCode closure test → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "classify",
    "title": "Add .github/ci-paths.json, its zod schema, glob matching, the release-only content rule and classification, and the ci-changes.ts entry writing GITHUB_OUTPUT for pull_request, push and workflow_dispatch with fail-open paths",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-6"],
    "paths": [".github/ci-paths.json", "tooling/src/ci-paths/**", "tooling/src/ci-changes.ts", "tools/toolu-conformance/src/harness/**"],
    "input": "Temp git repos built by the test: a base commit with real repo paths, then commits editing docs/statusline/README.md, README.md, tools/toolu-opencode/src/index.ts, newdir/file.txt, .github/workflows/tests.yml, .github/ci-paths.json, bun.lock, a #390-shaped version bump over every release-only file plus CHANGELOG.md, and a package.json scripts edit; real event JSON files for pull_request, push (real SHAs, all-zero before, bogus SHA) and workflow_dispatch; a real GITHUB_OUTPUT file",
    "check": "bun test --timeout 60000 tooling/src/ci-paths/__tests__/changes.test.ts",
    "model": "inherit"
  },
  {
    "id": "aggregate",
    "title": "Add the aggregate rules and the ci-aggregate.ts entry reading NEEDS (toJSON(needs)) and naming each failing job",
    "ac_refs": ["AC-1", "AC-2", "AC-5"],
    "depends_on": ["classify"],
    "paths": [".github/ci-paths.json", "tooling/src/ci-paths/**", "tooling/src/ci-aggregate.ts"],
    "input": "NEEDS JSON in GitHub's toJSON(needs) shape: docs-only pass, release-only all-skipped pass, changes failure, gate failure, opencode cancelled, gate skipped with ts=true, a missing gated job, an extra unmapped job",
    "check": "bun test --timeout 60000 tooling/src/ci-paths/__tests__/aggregate.test.ts",
    "model": "inherit"
  },
  {
    "id": "workflows",
    "title": "Rewrite tests.yml (changes, gated gate/opencode/docs, typescript aggregate via ci-aggregate) and toolu-review.yml (changes, review gated on changed with fail-open); drop paths-ignore; add check:ci-paths and test:docs scripts and wire check:ci-paths into test:ts; update the release-please token comment",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["aggregate"],
    "paths": [".github/**", "package.json", "tooling/src/opencode-acceptance/__tests__/workflow.test.ts", "tooling/src/__tests__/workspace-skeleton.test.ts", "tooling/src/ci-paths/**"],
    "input": "The real .github/workflows/*.yml parsed with Bun.YAML; the real package.json scripts",
    "check": "bun test --timeout 60000 tooling/src/opencode-acceptance/__tests__/workflow.test.ts tooling/src/__tests__/workspace-skeleton.test.ts tooling/src/ci-paths/__tests__/workflows.test.ts",
    "model": "inherit"
  },
  {
    "id": "check",
    "title": "Add the workflow/data consistency check (no workflow-level path filters on required-check workflows, aggregate needs = changes + gated jobs, gated if references its group, unmapped gated jobs, undefined groups, dead globs, unlisted gating workflows) and check-ci-paths.ts with --github override",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["workflows"],
    "paths": [".github/**", "tooling/src/ci-paths/**", "tooling/src/check-ci-paths.ts", "package.json"],
    "input": "The real repository (must pass) and sandbox copies of the real .github tree, each mutated once: paths-ignore added to tests.yml, the docs job mapping removed from ci-paths.json, typescript needs missing docs, a glob matching no tracked file, a gated job whose if names another group",
    "check": "bun run check:ci-paths && bun test --timeout 60000 tooling/src/ci-paths/__tests__/check.test.ts",
    "model": "inherit"
  },
  {
    "id": "closure",
    "title": "Test that the Bun.build import closure of the OpenCode acceptance entries and live tests is covered by the opencode group",
    "ac_refs": ["AC-8"],
    "depends_on": ["classify"],
    "paths": [".github/ci-paths.json", "tooling/src/**", "tools/toolu-opencode/**", "tools/toolu-conformance/**", "packages/toolu-core/**", "plugins/**"],
    "input": "The real source tree: tooling/src/opencode-*.ts and tools/toolu-opencode/**/*.live.test.ts bundled with metafile; failure case: the same closure checked against the real opencode globs minus tooling/src/env.ts must report tooling/src/env.ts as uncovered",
    "check": "bun test --timeout 120000 tooling/src/ci-paths/__tests__/closure.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Update the AGENTS.md CI table (jobs, groups, aggregate, review gating, no workflow-level filters) and docs/testing.md (test:docs, check:ci-paths); assert the table in workspace-skeleton",
    "ac_refs": ["AC-9"],
    "depends_on": ["check"],
    "paths": ["AGENTS.md", "docs/testing.md", "tooling/src/__tests__/workspace-skeleton.test.ts"],
    "input": "The real AGENTS.md and docs/testing.md",
    "check": "bun test --timeout 60000 tooling/src/__tests__/workspace-skeleton.test.ts && bun run test:docs",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the conventions gate (format, lint, typecheck, guardrails, knip, jscpd, gate reach, legacy exemptions) plus the docs job, the ci-paths tests and the bundle/workspace checks; run the full bun run test and compare failures with the origin/main baseline on this host",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8", "AC-9"],
    "depends_on": ["classify", "aggregate", "workflows", "check", "closure", "docs"],
    "paths": ["."],
    "input": "The whole branch",
    "check": "bun run test:conventions && bun run test:docs && bun test --timeout 120000 tooling/src/ci-paths tooling/src/opencode-acceptance/__tests__/workflow.test.ts && bun run test:workspace",
    "model": "inherit"
  }
]
```

## Critical files

- New:
  - `.github/ci-paths.json`
  - `tooling/src/ci-paths/{config,classify,aggregate,check}.ts`
  - `tooling/src/ci-changes.ts`, `tooling/src/ci-aggregate.ts`, `tooling/src/check-ci-paths.ts`
  - `tooling/src/ci-paths/__tests__/{changes,aggregate,workflows,check,closure}.test.ts`
- Modified:
  - `.github/workflows/tests.yml`, `.github/workflows/toolu-review.yml`, `.github/workflows/release-please.yml` (comment only)
  - `package.json`
  - `tooling/src/opencode-acceptance/__tests__/workflow.test.ts`, `tooling/src/__tests__/workspace-skeleton.test.ts`
  - `AGENTS.md`, `docs/testing.md`
  - `knip.json` or `tooling/gate-reach.json`, only if the gates require it

## Verification

- End to end: the `changes` script, run against real git repos and real event files, produces the spec's outputs for every AC-1…AC-6 input.
- The aggregate script passes and fails on real `toJSON(needs)` shapes.
- `check:ci-paths` passes on the repository and fails on each mutated copy of the real `.github`.
- The closure test proves that the `opencode` group covers what the acceptance imports.
- The workflow tests pin the YAML wiring.
- Boundary cases: all-zero `before`, a bogus SHA, an empty diff, dispatch, a release-only file with a non-version line, and renames.
- Docs: AGENTS.md and `docs/testing.md` are updated and asserted.
- Live: this PR touches `.github/**`, so its own run must start every job (fail open), and `typescript` and `review` must report.
- Full `bun run test`: failures are compared with an `origin/main` baseline on this host (memory be52369e), and CI is the authority.

Delivery: after `gate`, commit the scoped changes. Run `plan-ledger.js run <plan> --verify`,
then `toolu-review:review`; `verdict.js status` must report `overall: ready`. Then push, open
the PR (`Closes Falconiere/toolu#458`, `Part of Falconiere/toolu#402`) and hand off to
`pr-babysit:babysit`.

## Plan review

- closure: 🟡 should-fix: AC-8 had no failure case. Fixed: an uncovered-module case was added to the input.
- classify: 🔵 consider: the check reads the conformance harness. Fixed: added to `paths`.
- gate: 🟡 should-fix: `test:conventions` reads the whole tree, but `paths` listed only a subset. Fixed: `paths` is now `["."]`.
- Verification: 🔵 consider: the delivery order was implicit. Fixed: stated above.

AC coverage: AC-1…AC-9 each map to at least one step. Jev alignment came back 0.44 (inconclusive), so each AC was checked by hand against the step inputs.
