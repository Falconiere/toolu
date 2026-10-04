# OpenCode independently loadable npm package — Plan

**Date:** 2026-10-04   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-04-opencode-npm-package-design.md   **Topic:** OP-27 (#361)

## Evidence and approach

Inspected these files: `tools/toolu-opencode/package.json`, which declares `^7.4.0` core while v7.4.0 core lacks `dispatch/gates/startup/registry`; `tools/toolu-opencode/scripts/bundle-plugins.ts`, the prepack that stages 16 plugins; `tooling/src/pack-inventory.ts`, which runs `bun pm pack` against a hand list; `.github/workflows/npm-publish.yml`, which runs `npm publish`; `release-please-config.json`, whose extra-files bump `$.version` only; `tooling/src/opencode-host/scenarios-entry.ts`, whose `packTarball` is a hand stage with `--ignore-scripts`; and `scenarios-startup.ts`, whose `fullStartup` proves 16 plugins through the shim only. Measured: `npm pack --dry-run --json` lists the same 354 files as bun and reports modes, and the `!src/**/__tests__` negation leaves 290 files under both tools. Memory 6dbe117a: the live npm route installs core from the registry. Approach as in the approved spec: a content-derived closure gate, a manifest fix synced by release-please, a publish-path tarball, and one clean-install live scenario.

## Workstream summary

closure gate (red on today's manifest) → manifest and release sync (green) → inventory on npm → publish-path live pack and clean-install scenario → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "closure-gate",
    "title": "Add tooling/src/pack-closure.ts: reference, link, import, export, mode and forbidden-path rules over a packed {path, mode} list; fixture tests plus a real-package test that is red on today's manifest",
    "ac_refs": ["AC-2", "AC-3"],
    "paths": ["tooling/src/pack-closure.ts", "tooling/src/__tests__/pack-closure.test.ts", "tooling/src/pack-inventory.ts", "tools/toolu-opencode/**", "plugins/**", "packages/toolu-core/package.json"],
    "input": "The real tools/toolu-opencode package after prepack, listed by npm pack --dry-run --json; temp fixture packages written to disk, each breaking one rule (missing helper reference, broken link, outside relative import, undeclared bare import, unexported core subpath, missing export target, lost exec bit) plus placeholder and directory references",
    "check": "bun test --timeout 120000 tooling/src/__tests__/pack-closure.test.ts",
    "model": "inherit"
  },
  {
    "id": "manifest-release",
    "title": "Exclude __tests__ from files, set the @toolu/core floor to ^<version>, add optional SDK peers equal to the pin, send the prepack summary to stderr, add the release-please core-floor extra-file and the publish tag check, and extend the host contract check to the peers",
    "ac_refs": ["AC-1", "AC-4"],
    "depends_on": ["closure-gate"],
    "paths": ["tools/toolu-opencode/package.json", "tools/toolu-opencode/scripts/bundle-plugins.ts", "release-please-config.json", ".github/workflows/npm-publish.yml", "tooling/src/__tests__/npm-publish.test.ts", "tooling/src/opencode-host-contract.ts", "tooling/src/__tests__/opencode-host-contract.test.ts", "tooling/src/opencode-host/**", "tools/toolu-opencode/contract/**", "bun.lock"],
    "input": "The committed manifests, workflow and contract pin; the workflow's own tag-check run block executed by bash over temp copies of the three package.json files (matching tag exits 0; a tag that differs from the opencode core floor exits 1 naming it); the release-please jsonpath resolved against the real package.json to a semver string; a temp contract copy whose adapter peer differs from the pin",
    "check": "bun test --timeout 120000 tooling/src/__tests__/pack-closure.test.ts tooling/src/__tests__/npm-publish.test.ts tooling/src/__tests__/opencode-host-contract.test.ts && bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "inventory-npm",
    "title": "Switch pack-inventory to npm pack --dry-run --json for all three packages, run the closure gate for @toolu/opencode, forbid __tests__/fixtures, and replace the hand-listed helpers with the derived rules",
    "ac_refs": ["AC-1", "AC-2"],
    "depends_on": ["manifest-release"],
    "paths": ["tooling/src/pack-inventory.ts", "tooling/src/pack-closure.ts", "tooling/src/__tests__/**", "tools/toolu-opencode/**", "tools/toolu-cli/npm/**", "packages/toolu-core/**", "plugins/**"],
    "input": "The three real package directories packed by npm",
    "check": "bun run test:pack",
    "model": "inherit"
  },
  {
    "id": "live-clean-install",
    "title": "Build the live tarball through the real npm pack with prepack in a temp repo stage, and add the package.clean-install scenario: all 16 plugins over the npm route with TOOLU_REPO_ROOT and TOOLU_ROOT blank, helpers run from the installed tree, exports imported in a fresh bun, and the installed tree equal to the tarball",
    "ac_refs": ["AC-5", "AC-6"],
    "depends_on": ["inventory-npm"],
    "paths": ["tooling/src/opencode-host/**", "tooling/src/opencode-entry-smoke.ts", "tooling/src/__tests__/opencode-entry-smoke.test.ts", "tools/toolu-opencode/**", "plugins/**", "packages/toolu-core/**"],
    "input": "The pinned opencode-ai@1.18.34 CLI, the scripted loopback provider, a fresh publish-path tarball, and @toolu/core from the npm registry",
    "check": "bun test --timeout 60000 tooling/src/__tests__/opencode-entry-smoke.test.ts && bun run smoke:opencode-entry package.clean-install entry.npm-root surfaces.npm-clean surfaces.lifecycle cli.install",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Update docs/opencode.md (package proof and scenario list), AGENTS.md (inventory row and npm paragraph), the pack-inventory and npm-publish header comments, and regenerate the generated docs mirror",
    "ac_refs": ["AC-1", "AC-4", "AC-5"],
    "depends_on": ["live-clean-install"],
    "paths": ["docs/opencode.md", "AGENTS.md", "tools/toolu-opencode/generated/**", "tools/toolu-opencode/scripts/**", "plugins/**", "tooling/src/**", ".github/workflows/npm-publish.yml"],
    "input": "The edited docs and the real surface generator",
    "check": "bun run check:opencode-surface && bun run test:context-budget && bun test --timeout 120000 tools/toolu-opencode/scripts/__tests__",
    "model": "inherit"
  },
  {
    "id": "full-gate",
    "title": "Run the full repository gate",
    "ac_refs": ["AC-6"],
    "depends_on": ["docs"],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- New: `tooling/src/pack-closure.ts`, `tooling/src/__tests__/pack-closure.test.ts`, `tooling/src/opencode-host/scenarios-package.ts`
- Modified: `tools/toolu-opencode/package.json`, `tools/toolu-opencode/scripts/bundle-plugins.ts`, `tooling/src/pack-inventory.ts`, `tooling/src/opencode-host/scenarios-entry.ts`, `tooling/src/opencode-host/scenarios-startup.ts` (export the module and helper lists), `tooling/src/opencode-entry-smoke.ts`, `tooling/src/opencode-host-contract.ts`, `release-please-config.json`, `.github/workflows/npm-publish.yml`, `tooling/src/__tests__/npm-publish.test.ts`, `tooling/src/__tests__/opencode-host-contract.test.ts`, `docs/opencode.md`, `AGENTS.md`, `tools/toolu-opencode/generated/resources/repo/**`, `bun.lock` (only if the peer change alters it)

## Verification

- End to end: `bun run smoke:opencode-entry package.clean-install` on the pinned host passes with a tarball from the real `npm pack` with prepack, no checkout env, all 16 plugins ready, helpers and exports resolving from the installed tree, and the installed tree equal to the tarball.
- Failure and boundary: the closure fixtures each produce exactly one named problem; placeholders and directory references produce none; the real-package test fails before `manifest-release` (red evidence) and passes after; a peer pin drift fails `check:opencode-host`.
- Docs: `docs/opencode.md`, `AGENTS.md` and the generated mirror are synchronized (`check:opencode-surface`).
- Full `bun run test` is green, run through the epic job lease.
- Delivery: scoped commits, `plan-ledger.js run <plan> --verify` over the whole branch, `toolu-review:review` with version 2 state, `verdict.js status` reporting `overall: ready`, then push, open the PR against `main` (body starting `Closes Falconiere/toolu#361` / `Part of Falconiere/toolu#334`), and hand off to `pr-babysit:babysit`.

## Plan review

Jev alignment scored 1.44 (P = 0.53 for 'mapped but some checks would not observe the outcome').

- manifest-release: 🟡 should-fix: the publish tag check versus the core floor was observed only as a string in the workflow. Fixed: the step now executes the workflow's own run block against temp manifests, with a matching tag and a mismatched floor.
- Verification: 🟡 should-fix: no delivery sequence was listed. Fixed: added the final ledger verify, review, verdict, PR and babysit handoff.
- All six ACs are mapped (`AC-1` to `AC-6`). Every step has a runnable check with a real input. `paths` cover the tests and sources each check reads. Dependencies run in order. The docs step has a runnable check. Approved.
