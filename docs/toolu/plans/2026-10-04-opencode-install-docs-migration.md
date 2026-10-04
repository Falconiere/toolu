# OpenCode install docs and verified migration guide — Plan

**Date:** 2026-10-04   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-04-opencode-install-docs-migration-design.md   **Topic:** #363: replace the OpenCode install docs, add a migration guide, and tie support claims to CI evidence

## Evidence and approach

- **Inspected:**
  - docs: `docs/opencode.md`, `docs/opencode-host-contract.md`, `docs/conformance-report.md`, `docs/portable-core.md`, `docs/cli.md`, `README.md`, `docs/plugins/index.md`, `tools/toolu-opencode/README.md`;
  - harness: `tooling/src/opencode-host/{scenarios-entry,scenarios-cli,install-host,session,provider,host-run,doc-blocks}.ts`, `tooling/src/opencode-acceptance/{families,checks,report,live-tests}.ts`;
  - checks and CLI: `tooling/src/opencode-host-contract.ts`, `tooling/src/check-portable-core-doc.ts`, `tooling/src/__tests__/install-prompts.test.ts`, `tools/toolu-cli/src/opencode/{entries,update}.ts`, `tooling/src/clean-install-smoke.ts` (where V2 `plugin add` wrote).
- **Reuse:**
  - `entrySession`, `protectedWrite`, `enforced`, `GATED_FILES` and `npmSpec` (scenarios-entry);
  - `skills`, `catalogIds` and `host` (install-host);
  - the CLI build in scenarios-cli, exported rather than copied (jscpd threshold 0);
  - `readBlock`/`writeBlock` style markers (doc-blocks);
  - `acceptanceChecks` + `coverage` + `catalogNames` (acceptance);
  - `MatrixSchema`/`readJson`/`contractPaths` (opencode-host schema/results).
- **Design:** the approved spec.
  - Doc blocks run verbatim against the pinned host, with only `npx @toolu/plugins` and `opencode` redirected by PATH shims.
  - The support table is generated from the matrix and the acceptance registry.
  - A guard rejects stale V2 claims in the target-path docs.
  - The migration guide is a separate doc.

## Workstream summary

docs rewrite → doc-block runner and live `docs.*` scenarios → hermetic doc checker (support block, which lists the new checks, + stale guard) → live proof on the pinned host → regenerated mirror and AGENTS row → full gate and acceptance registration.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Rewrite target-path OpenCode docs and add docs/opencode-migration.md with executable quickstart/manage/migrate/rollback blocks",
    "check": "bun test tooling/src/__tests__/install-prompts.test.ts && bun run tooling/src/check-portable-core-doc.ts",
    "ac_refs": [
      "AC-6"
    ],
    "paths": [
      "README.md",
      "docs/plugins/index.md",
      "docs/opencode.md",
      "docs/opencode-migration.md",
      "docs/cli.md",
      "docs/portable-core.md",
      "docs/conformance-report.md",
      "tools/toolu-opencode/README.md",
      ".github/workflows/npm-publish.yml",
      "tooling/src/__tests__/install-prompts.test.ts",
      "tooling/src/check-portable-core-doc.ts"
    ],
    "input": "the repository docs as committed on origin/main",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Add doc-block extraction/runner and the docs.quickstart and docs.migration live scenarios; register family docs in acceptance and smoke:opencode-entry (implements AC-1/2/3/7; S4 proves them live)",
    "check": "bun test tooling/src/opencode-host/__tests__/doc-commands.test.ts",
    "depends_on": [
      "S1"
    ],
    "paths": [
      "tooling/src/opencode-host/doc-commands.ts",
      "tooling/src/opencode-host/scenarios-docs.ts",
      "tooling/src/opencode-host/scenarios-cli.ts",
      "tooling/src/opencode-host/__tests__/doc-commands.test.ts",
      "tooling/src/opencode-acceptance/families.ts",
      "tooling/src/opencode-entry-smoke.ts",
      "docs/opencode.md",
      "docs/opencode-migration.md"
    ],
    "input": "the real docs/opencode.md and docs/opencode-migration.md blocks; a shim dir exercised with a non-toolu npx package",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Add check:opencode-docs: generated per-plugin support block from capability-matrix.json + acceptance coverage, and the stale-V2 target-path guard; wire into test:portable-core; rendered after the docs.* checks join the registry so their coverage is in the table",
    "check": "bun test tooling/src/__tests__/check-opencode-docs.test.ts && bun run check:opencode-docs",
    "ac_refs": [
      "AC-4",
      "AC-5"
    ],
    "depends_on": [
      "S1",
      "S2"
    ],
    "paths": [
      "tooling/src/check-opencode-docs.ts",
      "tooling/src/opencode-acceptance/support-doc.ts",
      "tooling/src/__tests__/check-opencode-docs.test.ts",
      "tools/toolu-opencode/contract/capability-matrix.json",
      "tooling/src/opencode-acceptance",
      "tooling/src/opencode-host",
      "docs/opencode.md",
      "docs/opencode-migration.md",
      "README.md",
      "docs/cli.md",
      "docs/plugins/index.md",
      "tools/toolu-opencode/README.md",
      "package.json"
    ],
    "input": "the committed capability matrix, acceptance registry and real docs; sandbox copies with an edited row and injected V2 patterns",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Prove the documented steps on the pinned host",
    "check": "bun run smoke:opencode-entry docs.quickstart docs.migration docs.migration-refusals",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-7"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "docs/opencode.md",
      "docs/opencode-migration.md",
      "tooling/src/opencode-host",
      "tools/toolu-opencode",
      "tools/toolu-cli/src",
      "plugins"
    ],
    "input": "pinned opencode-ai@1.18.34, the packed @toolu/opencode tarball, the checkout CLI bundle, isolated HOME/XDG profiles, the scripted loopback provider",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Regenerate the OpenCode surface mirror and add the check:opencode-docs key-file row to AGENTS.md",
    "check": "bun run check:opencode-surface",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "S1",
      "S3"
    ],
    "paths": [
      "tools/toolu-opencode/generated",
      "AGENTS.md",
      "docs",
      "README.md",
      "plugins"
    ],
    "input": "the regenerated tools/toolu-opencode/generated tree",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Full repository gate",
    "check": "bun run test",
    "ac_refs": [
      "AC-4",
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "S3",
      "S4",
      "S5"
    ],
    "paths": [
      "."
    ],
    "input": "the whole branch",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "The docs checks run through the acceptance registry",
    "check": "bun run test:opencode --only docs.quickstart docs.migration docs.migration-refusals",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-7"
    ],
    "depends_on": [
      "S4",
      "S6"
    ],
    "paths": [
      "tooling/src/opencode-acceptance",
      "tooling/src/opencode-host",
      "docs/opencode.md",
      "docs/opencode-migration.md"
    ],
    "input": "the pinned host through the acceptance runner",
    "model": "inherit"
  }
]
```

## Critical files

- **Create:**
  - `docs/opencode-migration.md`
  - `tooling/src/check-opencode-docs.ts`
  - `tooling/src/opencode-acceptance/support-doc.ts`
  - `tooling/src/__tests__/check-opencode-docs.test.ts`
  - `tooling/src/opencode-host/doc-commands.ts`
  - `tooling/src/opencode-host/scenarios-docs.ts`
  - `tooling/src/opencode-host/__tests__/doc-commands.test.ts`
- **Modify:**
  - `docs/opencode.md`, `README.md`, `docs/plugins/index.md`, `docs/cli.md`, `docs/portable-core.md`, `docs/conformance-report.md`
  - `tools/toolu-opencode/README.md`, `.github/workflows/npm-publish.yml`
  - `tooling/src/opencode-host/scenarios-cli.ts` (export the CLI build), `tooling/src/opencode-acceptance/families.ts`, `tooling/src/opencode-entry-smoke.ts`
  - `package.json` (script and `test:portable-core`), `AGENTS.md`
  - `tools/toolu-opencode/generated/**` (regenerated)

## Verification

- **End to end.** The quick-start, manage, migrate and rollback blocks run verbatim on the pinned host in isolated profiles (S4, S7):
  - one `@toolu/opencode` entry;
  - `context7-context7` discovered and then removed;
  - `.env` write denied, with the bytes unchanged;
  - the V2 shim and dependency removed, with user comments and the other plugin kept;
  - a single load, and rollback restoring the seeded files byte for byte.
- **Failure and boundary cases:**
  - missing or duplicate markers make `docBlock` throw (S2 test);
  - the npx shim refuses other packages (S2 test);
  - an edited support row or matrix drift makes the checker exit 1 (S3 test);
  - an injected V2 pattern exits 1 with `file:line`, and the migration doc stays exempt (S3 test);
  - the model attempting no write fails the scenario (S4).
- **Documentation sync:** README/index prompt parity (S1), the portable-core doc check (S1), the regenerated mirror (S5), and `bun run test` (S6).
- Delivery: scoped commits per step (docs, checker, scenarios, mirror); `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run <plan> --verify` over the whole branch; `toolu-review:review`; `verdict.js status` = ready; push; PR body `Closes Falconiere/toolu#363` / `Part of Falconiere/toolu#334`; babysit.
- `opencode run`'s exit on a denied tool is observed in S4 and recorded here.

## Plan review

- Steps order: 🟡 should-fix: the support block renders acceptance coverage, but the checker step ran before the `docs.*` checks joined the registry, so its block would go stale (Jev order noul 0.69). Fixed: the runner and scenarios are S2, the checker is S3 and depends on S2 (re-check 0.72).
- S2 (runner): 🟡 should-fix: it claimed AC-1/2/3/7 with a hermetic check that cannot show the live outcome, and it ran a repo-wide typecheck with narrow `paths`. Fixed: no `ac_refs` (S4 and S7 prove those ACs live); typecheck left to S6.
- Verification: 🟡 should-fix: the delivery sequence was missing. Fixed: scoped commits, final `run --verify`, review, verdict, push and PR handoff.
- AC coverage: per-AC Jev nouls are AC-1 0.86, AC-2 0.88, AC-3 0.86, AC-4 0.91, AC-5 0.70, AC-6 0.88, AC-7 0.89. Every AC is mapped by `ac_refs` (S1, S3–S7). The low aggregate score (0.46) came from the stricter "every" phrasing; each AC was checked by hand against its step's check and input.

**Status:** Approved

## Deviations

- **Open question resolved (S4).** `opencode run` exits 0 after its only tool call is denied: `docs.quickstart` ran the block under `bash -euo pipefail` with `startExit: 0`. The quick-start block therefore needs no `|| true`.
- **AC-7 observation.** The `manage` block runs as one script, so `docs.quickstart` observes its end state rather than the state between commands:
  - the selection is `toolu`, so `remove context7` ran;
  - no toolu skill and no `@toolu/opencode` entry remain;
  - `update` reported `current`.

  `cli.lifecycle` already shows a disabled plugin's skill disappearing after `remove`.
- **Migrate block (S4 finding).** The first live run failed `keptUnchanged`. The CLI explains it: a bare `npx @toolu/plugins install --host opencode` with an existing project selection enables every catalog plugin in that file. The block now runs `update || install toolu`, which leaves the selection alone (Jev choice U, 0.82 over `install toolu` + `update`). The scenario now names the kept files that changed (`keptChanged`).
- **Review round 1 (toolu-review).** The reviewer found nine issues (one bug, six risks, one nit, one question), and Jev triaged them at 0.44–0.94. All were fixed:
  - the quick start uses a scratch `.env.toolu-check`;
  - the migrate block keeps private, write-once backups and stops on any `update` failure;
  - the new `docs.migration-refusals` check proves the exit-1 and exit-2 paths leave user files unchanged, and S4/S7 now run it;
  - the shims are single-quoted;
  - the renderer's branches have tests.
