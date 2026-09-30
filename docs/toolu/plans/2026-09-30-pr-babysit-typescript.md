# pr-babysit Bun port — Plan

**Date:** 2026-09-30   **Status:** Approved   **Spec:** docs/toolu/specs/2026-09-30-pr-babysit-typescript-design.md   **Topic:** Replace the scoped pr-babysit bash commands and libs with Bun bundles.

## Evidence and approach

Issue #273 names seven CLI scripts and five shared libs. The existing `scripts/__tests__` holds real captured PR snapshots and `gh` pages. `babysit-tick.sh` owns persistence order, `collect-pr.sh` owns fan-out and head verification, `reduce-state.sh` owns closed decisions, and the three write commands own all side effects. Preserve these boundaries in TypeScript, compare against the original bash with isolated fixtures before deleting it, and keep every test offline. Use seven top-level `hooks/src/babysit-*.ts` entries and `hooks/src/babysit/` helpers so the existing Bun bundle builder produces `hooks/dist/babysit-*.js` without a new pipeline. The separate fixer scripts still source three named libs; move those shell functions to a fixer-specific compatibility helper and change only their source statements.

## Workstream summary

Port pure parsing and normalization, then the collector and reducer, then the tick and write side. Replace callers and tests, verify both hosts in temporary profiles, and deliver through the full gate and PR flow.

## Steps (machine-readable)

```json
[
  {
    "id": "verdict",
    "title": "Port verdict parsing and recorded-comment tests",
    "ac_refs": ["AC-1"],
    "paths": ["plugins/pr-babysit/scripts/parse-verdict.sh", "plugins/pr-babysit/hooks/src/**", "plugins/pr-babysit/scripts/__tests__/fixtures/**"],
    "input": "Captured PR #31, #120, #122, #157, provider-error and in-progress comments",
    "check": "bun test plugins/pr-babysit/hooks/src/__tests__/parse-verdict.test.ts"
  },
  {
    "id": "collect",
    "title": "Port bounded gh transport, normalization and collection",
    "ac_refs": ["AC-2"],
    "depends_on": ["verdict"],
    "paths": ["plugins/pr-babysit/scripts/collect-pr.sh", "plugins/pr-babysit/scripts/lib/gh.sh", "plugins/pr-babysit/scripts/lib/normalize.sh", "plugins/pr-babysit/hooks/src/**", "plugins/pr-babysit/scripts/__tests__/fixtures/**"],
    "input": "Recorded pages for PR #115/#165 and stub gh responses, including 404 and moving head",
    "check": "bun test plugins/pr-babysit/hooks/src/__tests__/collect-pr.test.ts"
  },
  {
    "id": "reduce",
    "title": "Port pure state reduction and captured-snapshot parity",
    "ac_refs": ["AC-3"],
    "depends_on": ["verdict"],
    "paths": ["plugins/pr-babysit/scripts/reduce-state.sh", "plugins/pr-babysit/scripts/lib/normalize.sh", "plugins/pr-babysit/hooks/src/**", "plugins/pr-babysit/scripts/__tests__/fixtures/snapshots/**"],
    "input": "Recorded snapshots for toolu #115/#165 and comemory #216 with prior-state variants",
    "check": "bun test plugins/pr-babysit/hooks/src/__tests__/reduce-state.test.ts"
  },
  {
    "id": "tick",
    "title": "Port tick, atomic IO and slot lock",
    "ac_refs": ["AC-4"],
    "depends_on": ["collect", "reduce"],
    "paths": ["plugins/pr-babysit/scripts/babysit-tick.sh", "plugins/pr-babysit/scripts/lib/common.sh", "plugins/pr-babysit/scripts/lib/lock.sh", "plugins/pr-babysit/hooks/src/**", "plugins/pr-babysit/scripts/__tests__/fixtures/snapshots/**"],
    "input": "Captured toolu #165 snapshot and temporary malformed/foreign/locked states",
    "check": "bun test plugins/pr-babysit/hooks/src/__tests__/babysit-tick.test.ts"
  },
  {
    "id": "writes",
    "title": "Port reply, resolve and record with offline write tests",
    "ac_refs": ["AC-5"],
    "depends_on": ["tick"],
    "paths": ["plugins/pr-babysit/scripts/reply-thread.sh", "plugins/pr-babysit/scripts/resolve-thread.sh", "plugins/pr-babysit/scripts/record.sh", "plugins/pr-babysit/scripts/lib/state.sh", "plugins/pr-babysit/hooks/src/**", "plugins/pr-babysit/scripts/__tests__/fixtures/**"],
    "input": "Recorded state with stub gh on PATH returning comment ids, false and true resolve responses",
    "check": "bun test plugins/pr-babysit/hooks/src/__tests__/write-side.test.ts"
  },
  {
    "id": "docs-and-delivery",
    "title": "Replace all callers, delete scoped bash and bats, verify bundles, docs and hosts",
    "ac_refs": ["AC-6"],
    "depends_on": ["verdict", "collect", "reduce", "tick", "writes"],
    "paths": ["plugins/pr-babysit/**", "docs/pr-babysit/**", "docs/gate-coverage-matrix.md", "tooling/fixtures/gate-coverage/inventory.json", "tooling/src/pr-babysit-herdr-smoke.ts", "tools/toolu-opencode/**"],
    "input": "Installed plugin in isolated Codex and Claude profiles, with a space in each path; recorded snapshots and stub gh only",
    "check": "bun run check:plugin-bundles && bun run test"
  }
]
```

## Critical files

`plugins/pr-babysit/hooks/src/babysit-*`, `plugins/pr-babysit/hooks/src/babysit/*`, committed `hooks/dist/babysit-*.js` bundles, the seven named shell entrypoints and five named libs, eleven related bats suites, `plugins/pr-babysit/scripts/{route-fix,dispatch-fix,fixer-report}.sh` and fixer-specific compatibility helper, `plugins/pr-babysit/{README.md,commands/babysit.md,skills/babysit/SKILL.md,skills/babysit/references/helper.md,workflows/babysit.md}`, `docs/pr-babysit/README.md`, gate coverage inventory/matrix, and other direct callers found by exact path search.

## Verification

Before deletion, replay the recorded fixture cases against bash and TypeScript in isolated temporary directories and compare JSON, exit codes and persistent file bytes. Stub `gh` for all collector/write tests; no test targets a real PR, issue or thread. Then run the targeted Bun suites, bundle drift and `bun run test` in the foreground with a timeout, host installs in temporary profiles, final ledger `--verify`, review and verdict gates. Commit and push after each tested stage; rebase on a moved `origin/main` before implementation and delivery. Open a `main` PR with the required issue links, then babysit without merging.
