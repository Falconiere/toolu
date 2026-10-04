# OpenCode Rust post-edit quality (OP-20) — Plan

**Date:** 2026-10-04 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-04-opencode-rust-quality-design.md **Topic:** Prove selected rust-quality enforcement on native OpenCode edits.

## Evidence and approach

The OP-06 bridge already sends completed OpenCode `write`, `edit` and `apply_patch` calls to `dispatchPostTool`, and OP-18 (#352) made the bridge walk every completed patch path. OP-19 (#353, PR #389) proved python-quality on that bridge with no production change. The shipped rust-quality module (`plugins/rust-quality/hooks/src/post-tool-use.ts`) has the same shape: it needs `Cargo.toml` at the git toplevel and `cargo` on `PATH`, checks `.rs` files through `fileQuality` and does not skip linked worktrees. Its error-handling and no-mocks rules run real `ast-grep scan` Rust rules and key on `/src/` and `/tests/` in the path as given. Add a real-bundle adapter test, a pinned-host smoke built on `quality-smoke-shared.ts`, and documentation. Change production code only if the adapter test exposes a defect.

## Workstream summary

Real-bundle adapter test → pinned-host smoke → documentation → full quality gate.

## Steps (machine-readable)

```json
[
  {
    "id": "bundle",
    "title": "Run the committed rust-quality module through the OpenCode post adapter on real Rust edits and boundaries",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["tools/toolu-opencode/src/adapter/**", "plugins/rust-quality/**", "packages/toolu-core/src/**", "tools/toolu-conformance/**"],
    "input": "Sandboxed git projects with Cargo.toml, real cargo, git and ast-grep, and the committed registry bundle; native write/edit and an add/update/move/delete patch with two violating destinations and a .md; clean recovery; disabled selection, missing Cargo.toml, PATH without cargo; two projects; a linked worktree; commit/push gate decisions",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/rust-quality-post.test.ts plugins/rust-quality/hooks/src/__tests__",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Execute selected rust-quality on the pinned OpenCode host and inspect bytes, model text, gate state and later refusals",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["bundle"],
    "paths": ["tooling/src/**", "tools/toolu-opencode/**", "plugins/rust-quality/**", "plugins/toolu/**", "packages/toolu-core/src/**", "package.json"],
    "input": "Exact opencode-ai and plugin SDK pin in isolated profiles with a loopback scripted provider; native write/edit/patch calls, selected and disabled configurations, invalid Rust, commit/push marker commands, real file and gate inspection",
    "check": "bun run smoke:opencode-rust-quality",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the proven OpenCode rust-quality behavior and synchronize generated resource mirrors",
    "ac_refs": ["AC-1", "AC-4", "AC-5"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "Observed behavior and prerequisites from the adapter and pinned-host runs, including after-execution diagnostics, path-dependent rules and linked-worktree behavior",
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

Add `tools/toolu-opencode/src/adapter/__tests__/rust-quality-post.test.ts`, `tooling/src/opencode-host/scenarios-rust-quality-smoke.ts`, `tooling/src/opencode-rust-quality-smoke.ts`, and the `smoke:opencode-rust-quality` root script. Update `plugins/rust-quality/README.md`, `docs/rust-quality/README.md`, `docs/opencode.md`, `docs/opencode-host-contract.md` and their generated OpenCode resource mirrors. Change `plugins/rust-quality/hooks/src/`, the bridge or the dispatcher only if a real failing test identifies a defect, and rebuild the committed bundle if so.

## Verification

Inspect real file bytes, actual ast-grep Rust findings (`.unwrap()`, `.expect()`, `mockall` import), per-file quality entries, model-visible result text and absent commit/push markers. Cover delete/move cleanup, unrelated files, selection, the `Cargo.toml` and `cargo` prerequisites, linked-worktree checking and project isolation. Show the adapter test can fail: with the move source no longer treated as removed and rust-quality skipping linked worktrees (both temporary, rebuilt, recorded, not committed), the patch and linked-worktree cases must fail. Keep the pinned-host result distinct from adapter evidence. Run focused tests, host-contract and surface checks, the full `bun run test` gate, final ledger `run --verify`, the committed-diff review and verdict readiness before push and PR.

Fetch and rebase on `origin/main` before implementation and before pushing if main moved; re-run affected steps after a rebase. Commit each tested increment; the PR and babysit handoff follow the final delivery checks and require an authenticated `gh api user` call on the non-default branch `feat/354-opencode-port-rust-quality-post`.

## Plan review

**Status:** Approved. All five spec ACs are referenced with no dangling IDs (checked by diffing spec AC ids against ledger `ac_refs`). Each step has a runnable check against real inputs, including the disabled, missing-`Cargo.toml`, missing-`cargo`, unrelated-file and linked-worktree boundaries; the mutation check shows the adapter test can fail. `live` depends on `bundle`, `docs` on `live`, `gate` on `docs`. Jev scored executability 1.69 of 2 (confidence 0.54); direct review found only the missing delivery prerequisite, now stated above, and confirmed the `bundle` check also runs rust-quality's existing golden suites as the existing-host regression.
