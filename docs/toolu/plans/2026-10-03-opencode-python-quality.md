# OpenCode Python post-edit quality (OP-19) — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-python-quality-design.md **Topic:** Prove selected python-quality enforcement on native OpenCode edits.

## Evidence and approach

The OP-06 bridge already sends completed OpenCode `write`, `edit` and `apply_patch` calls to `dispatchPostTool`, and OP-18 (#352) made the bridge walk every completed patch path. The shipped python-quality module needs a Python marker at the git toplevel and `python3`, checks `.py` files through `fileQuality` and does not skip linked worktrees. A probe adapter test against the committed bundle passed on the first run for writes, edits, a move/delete patch, selection, prerequisites, two projects and a linked worktree, so no production change is planned. Add that test, a pinned-host smoke modeled on `scenarios-ts-quality-smoke.ts`, and documentation.

## Workstream summary

Real-bundle adapter test → pinned-host smoke → documentation → full quality gate.

## Steps (machine-readable)

```json
[
  {
    "id": "bundle",
    "title": "Run the committed python-quality module through the OpenCode post adapter on real Python edits and boundaries",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["tools/toolu-opencode/src/adapter/**", "plugins/python-quality/**", "packages/toolu-core/src/**", "tools/toolu-conformance/**"],
    "input": "Sandboxed git projects with pyproject.toml, real python3, git and ast-grep, and the committed registry bundle; native write/edit and an add/update/move/delete patch with two violating destinations and a .md; clean recovery; disabled selection, missing marker, PATH without python3; two projects; a linked worktree; commit/push gate decisions",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/python-quality-post.test.ts plugins/python-quality/hooks/src/__tests__",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Execute selected python-quality on the pinned OpenCode host and inspect bytes, model text, gate state and later refusals",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["bundle"],
    "paths": ["tooling/src/**", "tools/toolu-opencode/**", "plugins/python-quality/**", "plugins/toolu/**", "packages/toolu-core/src/**", "package.json"],
    "input": "Exact opencode-ai and plugin SDK pin in isolated profiles with a loopback scripted provider; native write/edit/patch calls, selected and disabled configurations, invalid Python, commit/push marker commands, real file and gate inspection",
    "check": "bun run smoke:opencode-python-quality",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the proven OpenCode python-quality behavior and synchronize generated resource mirrors",
    "ac_refs": ["AC-1", "AC-4", "AC-5"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "Observed behavior and prerequisites from the adapter and pinned-host runs, including after-execution diagnostics and linked-worktree behavior",
    "check": "bun run check:opencode-surface && bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch",
    "ac_refs": ["AC-4", "AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "Whole test and documentation diff with focused and pinned-host checks green",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Add `tools/toolu-opencode/src/adapter/__tests__/python-quality-post.test.ts`, `tooling/src/opencode-host/scenarios-python-quality-smoke.ts`, `tooling/src/opencode-python-quality-smoke.ts`, and the `smoke:opencode-python-quality` root script. Update `plugins/python-quality/README.md`, `docs/python-quality/README.md`, `docs/opencode.md`, `docs/opencode-host-contract.md` and their generated OpenCode resource mirrors. Change `plugins/python-quality/hooks/src/`, the bridge or the dispatcher only if a real failing test identifies a defect, and rebuild the committed bundle if so.

## Verification

Inspect real file bytes, actual ast-grep Python findings, per-file quality entries, model-visible result text and absent commit/push markers. Cover delete/move cleanup, unrelated files, selection, marker and `python3` prerequisites, linked-worktree checking and project isolation. Show the adapter test can fail: with the bridge's `continuePostBlocks` temporarily off, the patch case must lose its second diagnostic (recorded, not committed). Keep the pinned-host result distinct from adapter evidence. Run focused tests, host-contract and surface checks, the full `bun run test` gate, final ledger `run --verify`, the committed-diff review and verdict readiness before push and PR.

Fetch and rebase on `origin/main` before implementation and before pushing if main moved; re-run affected steps after a rebase. Commit each tested increment; the PR and babysit handoff follow the final delivery checks.

## Plan review

**Status:** Approved. All five spec ACs are referenced with no dangling IDs. Each step has a runnable check against real inputs, including the disabled, missing-marker, missing-`python3`, unrelated-file and linked-worktree boundaries; the mutation check shows the adapter test can fail. `live` depends on `bundle`, `docs` on `live`, `gate` on `docs`. Jev judged the plan executable (0.71); direct review confirmed the `bundle` check also runs python-quality's existing golden suites as the existing-host regression.
