# OpenCode post-tool checks (OP-06) — Plan

**Date:** 2026-10-02 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-02-opencode-post-tool-checks-design.md **Topic:** Dispatch completed OpenCode tool results through native post checks.

## Evidence and approach

The approved spec and OP-01 probe establish that pinned OpenCode 1.18.34 calls `tool.execute.after` for completed edits and nonzero shell exits but not thrown tool errors. OP-03's `mapToolCall` gives core names and inputs; `dispatchPostTool` already runs gate-status, push-waiver, selected registry modules, and per-path patch checks. OP-05's advice store supplies the model-visible result channel. Reuse those contracts, require a verified shell outcome before state-changing dispatch, and keep one bounded per-call completion guard.

## Workstream summary

Post result bridge and subprocess proof → pinned-host proof → lifecycle and documentation → full gate and delivery audit.

## Steps (machine-readable)

```json
[
  {
    "id": "bridge",
    "title": "Normalize final host results and dispatch post checks once; compose advice and post diagnostics without a rollback claim",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**",
      "packages/toolu-core/src/dispatch/**",
      "packages/toolu-core/src/gates/**",
      "packages/toolu-core/src/quality/**",
      "plugins/toolu/hooks/src/post-tools/**",
      "plugins/ts-quality/hooks/dist/post-tool-use.js"
    ],
    "input": "Real temp git/TypeScript projects, native quality command exit 3 then 0, real selected post registry bundle, edit/write and add/update/delete/move patch; missing/invalid/interrupted shell outcome, malformed payload, duplicate and cross-session IDs",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__/tool-post.test.ts tools/toolu-opencode/src/adapter/__tests__/tool-advice.test.ts tools/toolu-opencode/src/plugin/__tests__/hooks.test.ts",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Prove post diagnostics, quality state and later commit/push refusal on the pinned OpenCode host",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-5"],
    "depends_on": ["bridge"],
    "paths": [
      "tooling/src/opencode-posttool-smoke.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/contract/**",
      "plugins/toolu/**",
      "plugins/ts-quality/**",
      "package.json"
    ],
    "input": "Pinned opencode-ai@1.18.34 in isolated git project/profile with scripted loopback provider; actual edit/patch and shell quality exit 3, model tool-result text, quality gate file, later commit/push denial, failed read with no after",
    "check": "bun run smoke:opencode-posttool",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Update lifecycle support and document verified post-tool semantics with synchronized OpenCode resource mirrors",
    "ac_refs": ["AC-3", "AC-4", "AC-5"],
    "depends_on": ["live"],
    "paths": ["**"],
    "input": "Observed pinned-host results, lifecycle table, docs/opencode.md and docs/opencode-host-contract.md plus generated OpenCode resources",
    "check": "bun test tools/toolu-opencode/src/lifecycle/__tests__/lifecycle.test.ts && bun run check:opencode-surface && bun run test:portable-core",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch and prepare the committed-diff delivery audit",
    "ac_refs": ["AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "Whole implementation and documentation branch with the real-host smoke and affected subprocess tests green",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Add `tools/toolu-opencode/src/adapter/tool-post.ts` and a colocated test. Modify `tools/toolu-opencode/src/adapter/{tool-before,tool-advice}.ts` only as needed for call lifecycle; wire `tools/toolu-opencode/src/plugin/{enforcement,hooks}.ts`. Add a pinned-host scenario and runner under `tooling/src/opencode-host/` and `tooling/src/opencode-posttool-smoke.ts`, with a root script. Update `tools/toolu-opencode/src/lifecycle/table.ts` and its test, `docs/opencode.md`, `docs/opencode-host-contract.md`, and generated resource mirrors. Core dispatcher and existing hosts should need no behavior change.

## Verification

The direct bridge test must observe real gate file bytes after shell exit 3/0 and per-file quality checks after edits, patch moves, and deletes. It must prove no pass or waiver from unknown/interrupted results and one diagnostic for a duplicate after. The pinned host smoke must observe model-visible diagnostics and real later commit/push denial with target side effects absent. Run the full gate, final ledger `--verify`, committed-diff review, verdict readiness, and delivery preflight before the final push and PR handoff. Re-run affected checks after a rebase.

## Execution variance

The orchestrator authorized a PR handoff if the pinned host stalls before a provider request even without toolu. On this worker, a host-only isolated baseline timed out after 600 seconds with zero provider requests or hook events. The OP-06 smoke also timed out after 300 seconds at the same config-loading point after the exact pinned SDK was installed in both isolated config directories. This leaves the live step unverified here. The smoke remains in the branch for CI and OP-28's mandatory live acceptance; run the docs and full repository gates directly and disclose both host results in the PR. Do not mark the live ledger step green from these attempts.

The local `bun run test` reached the broad unit suite after conventions passed, then the unchanged registry import-cost test measured 193.8 ms against its 50 ms sanity bound. The same test measured 101.7 ms when run alone on this host. The later gate stages were run individually: OpenCode bridge 23/23, portable-core 54/54, gate coverage, final removal, bundle and hook drift, workspace, pack, conformance, context budgets, and deterministic benchmarks passed. The unchanged shell benchmark exceeded its 100 µs p99 bound at 186.1 µs, then 134.3 µs with CPU affinity. The final conventions and generated-surface checks passed after the smoke cache fix. Neither timing bound was changed; CI will arbitrate these local timing failures.

## Plan review

Status: Approved. `checkAcRefs` found no dangling references; each AC has a runnable step and the dependency order is acyclic. The bridge step includes boundary inputs for partial and repeated calls, the live step inspects host-visible output and persisted state, and the final gate covers the whole repository. Jev's semantic alignment rating was uncertain (0.52), so the review checked each AC and input against the approved spec rather than treating the rating as approval.
