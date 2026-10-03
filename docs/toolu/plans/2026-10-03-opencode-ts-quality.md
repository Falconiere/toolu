# OpenCode TypeScript post-edit quality (OP-18) — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-ts-quality-design.md **Topic:** Prove and repair selected ts-quality enforcement on native OpenCode edits.

## Evidence and approach

The approved spec and OP-06 bridge already connect completed OpenCode tools to `dispatchPostTool`. The shipped ts-quality module uses tracked `tsconfig.json`, a real package manager, `fileQuality` and optional real `ast-grep`. One adapter test runs that bundle on a write; the OP-06 live smoke covers a write and patch but does not prove this issue's selection, move/delete, toolchain and commit/push matrix. A red patch test showed the shared post walk stops at its first violation and leaves later completed edits unchecked. Add OpenCode-only post-block continuation, issue-owned tests and no unrelated behavior change. Keep the pinned host contract, existing hosts and documented linked-worktree behavior.

## Workstream summary

Real bundle integration → pinned-host execution → user documentation → full quality gate and delivery audit.

## Steps (machine-readable)

```json
[
  {
    "id": "bundle",
    "title": "Continue OpenCode post patch checks after a block, then run the committed ts-quality module on real TypeScript edits and boundaries",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["**"],
    "input": "Isolated git projects with tracked tsconfig, bun.lock, actual ast-grep and the committed registry bundle; native write/edit and add/update/move/delete patch calls; two blocking destinations in one patch, clean recovery, disabled selection, missing markers, linked worktree and separate project state; existing core default still short-circuits",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts tools/toolu-opencode/src/adapter/__tests__/tool-post.test.ts packages/toolu-core/src/dispatch/__tests__/dispatch-native.test.ts plugins/ts-quality/hooks/src/__tests__/register.test.ts",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Execute selected ts-quality on the pinned OpenCode host and inspect bytes, model text, gate state and later refusals",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["bundle"],
    "paths": ["**"],
    "input": "Exact opencode-ai and plugin SDK pin in isolated profiles with loopback scripted provider; native write/edit/patch calls, selected and disabled configurations, invalid TypeScript, commit/push marker commands, real file and gate inspection",
    "check": "bun run smoke:opencode-ts-quality",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the proven OpenCode ts-quality behavior and synchronize generated resource mirrors",
    "ac_refs": ["AC-1", "AC-4", "AC-5"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "Observed behavior and prerequisites from the bundle and pinned-host runs, including after-execution diagnostics and linked-worktree behavior",
    "check": "bun run check:opencode-surface && bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch and prepare the committed-diff delivery review",
    "ac_refs": ["AC-4", "AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "Whole implementation and documentation diff with focused and pinned-host checks green",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Add `tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts` beside the existing post-adapter tests and add a focused core dispatcher test. Add `continuePostBlocks` to `packages/toolu-core/src/dispatch/dispatch.ts`, pass it from `tools/toolu-opencode/src/adapter/tool-post.ts`, and preserve the omitted/default behavior. Add `tooling/src/opencode-host/scenarios-ts-quality-smoke.ts` and `tooling/src/opencode-ts-quality-smoke.ts`, plus the root package script. Change `plugins/ts-quality/hooks/src/` only if another real failing test identifies a defect; rebuild its committed bundle if changed. Update `plugins/ts-quality/README.md`, `docs/ts-quality/README.md`, `docs/opencode.md`, `docs/opencode-host-contract.md`, and their generated mirrors.

## Verification

Inspect real file bytes, actual ast-grep findings, per-file quality entries, model-visible result text and absence of denied commit/push side effects. Verify delete/move cleanup, selection, prerequisites, linked-worktree skip and project isolation. Keep the pinned live host result distinct from subprocess evidence. Run focused tests, host-contract and surface checks, the full `bun run test` gate, final ledger verification, committed-diff review and verdict readiness before push and PR.

Fetch and rebase on `origin/main` before implementation and again before pushing if main moved. Re-run every affected step after a rebase. Commit each tested work increment and push this branch after its required review state is recorded; the PR and babysit handoff follow only after the final delivery checks.

## Plan review

**Status:** Approved. The machine-readable ledger parses; all five spec ACs are referenced with no dangling IDs. The real-input checks cover failure, recovery and selection boundaries, and `paths: ["**"]` keeps transitive code and fixtures in the freshness calculation. Jev judged the amendment uncertain (0.29); direct review confirmed the bundle command includes both the new OpenCode patch test and a core default-behavior regression test, before the dependent live step.

## Deviations

The first real-bundle patch test failed because `dispatchRecords` stopped on the first post block. The initial plan expected to reuse the shared walk unchanged. The bundle step now includes an OpenCode-only continuation option plus a core default-behavior regression check. After that repair, the test exposed another gap: the existing `language: ts` ast-grep rules return no match on `.tsx`, while `language: tsx` matches the empty catch. The bundle step includes OpenCode-only TSX rule selection and the same real-file adapter check. Jev preferred the host-scoped leaf fix over changing existing hosts or the shared scanner.

The repository's 300-line test-file limit required moving the new adapter cases into a colocated `ts-quality-post.test.ts`. The bundle check includes both that file and the existing `tool-post.test.ts` regression suite.

PR review exposed a mixed-result gap: an earlier post block hid a later registry crash warning from OpenCode's model text. The follow-up keeps fatal nonzero exits terminal and includes stderr beside an aggregated block, with real adapter and dispatcher regression cases.
