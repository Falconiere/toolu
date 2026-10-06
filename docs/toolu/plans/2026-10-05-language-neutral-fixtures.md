# Language-neutral parity fixtures — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-language-neutral-fixtures-design.md   **Topic:** #408 shared JSON fixture contracts

## Evidence and approach

#408 names five existing JSON trees and the TypeScript tables to export. The reviewed spec sets the JSON paths, baseline counts, ordered setup descriptors, and failure behavior. The current conformance harness owns real sandbox creation; its case-specific functions become a bounded interpreter over committed JSON. Existing golden comparisons remain the behavioral oracle. The three tooling-only fixture trees stay put until #439. The host is root, so known permission tests need comparison with a clean base checkout if the full local gate reports the known environmental failures.

## Workstream summary

Inventory and loader → existing JSON moves → gate and conformance records → quality and ast-grep records → future-port records and shell parser baseline → fixture docs, full gate and delivery audit.

## Steps (machine-readable)

```json
[
  {
    "id": "inventory-loader",
    "title": "Record the 13 exported case arrays' original names/counts and add strict JSON case loading plus bounded sandbox action and path-token interpretation",
    "ac_refs": ["AC-2", "AC-3", "AC-4", "AC-5", "AC-7"],
    "paths": ["fixtures/index.json", "tools/toolu-conformance/src/harness/json-cases.ts", "tools/toolu-conformance/src/harness/__tests__/json-cases.test.ts", "tooling/src/check-fixture-inventory.ts", "tooling/src/__tests__/check-fixture-inventory.test.ts"],
    "input": "The 13 original exported case arrays (979 names); valid records plus unknown operation/token, duplicate name, missing capture, and sandbox escape cases in real temporary directories",
    "check": "bun test --timeout 60000 tools/toolu-conformance/src/harness/__tests__/json-cases.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts",
    "model": "inherit"
  },
  {
    "id": "existing-json",
    "title": "Move the five existing JSON contract trees to fixtures and update all TypeScript, documentation, and CI readers",
    "ac_refs": ["AC-1"],
    "depends_on": ["inventory-loader"],
    "paths": ["fixtures/shell/**", "fixtures/config/**", "fixtures/portable-core/**", "fixtures/gate-coverage/**", "fixtures/codex-hook-schemas/**", "packages/toolu-core/src/**", "tooling/src/**", "tools/toolu-conformance/src/**", "tools/toolu-opencode/src/**", ".github/ci-paths.json", "docs/**"],
    "input": "Existing bats-parity, issue-283, fail-closed config, protected-files pre-tool, gate inventory, and Codex hook output schemas; old-path references must be absent",
    "check": "bun test --timeout 60000 packages/toolu-core/src/shell/__tests__ packages/toolu-core/src/config/__tests__ tools/toolu-conformance/src/cli/__tests__ && bun run test:gate-coverage && bun run test:docs",
    "model": "inherit"
  },
  {
    "id": "lifecycle-gates",
    "title": "Export lifecycle and pre-tool modules A/B/C, including push-review and #283 cases, and move their bash golden captures beside the records",
    "ac_refs": ["AC-2", "AC-7"],
    "depends_on": ["inventory-loader"],
    "paths": ["fixtures/gates/**", "plugins/toolu/hooks/src/__tests__/**", "plugins/toolu/hooks/dist/**", "packages/toolu-core/src/**", "tools/toolu-conformance/src/harness/**"],
    "input": "117 lifecycle, 109 A, 110 B, and 180 C named cases; protected .env, malformed config, push-review and #283 negatives, and their existing bash captures",
    "check": "bun test --timeout 60000 plugins/toolu/hooks/src/__tests__",
    "model": "inherit"
  },
  {
    "id": "conformance-corpora",
    "title": "Export the pre-tool edit/shell and post-tool corpus records with ordered real sandbox setup and unchanged host decisions",
    "ac_refs": ["AC-3", "AC-7"],
    "depends_on": ["inventory-loader", "lifecycle-gates"],
    "paths": ["fixtures/gates/pretool-corpus.json", "fixtures/gates/posttool-corpus.json", "tools/toolu-conformance/src/**", "plugins/toolu/hooks/dist/**", "packages/toolu-core/src/**"],
    "input": "33 pre-tool and 16 post-tool cases, including malformed stdin, multi-path patch, pending push waiver, registry exit 2, and Claude/Codex outcomes",
    "check": "bun run test:conformance && bun test --timeout 60000 plugins/toolu/hooks/src/__tests__/pre-tools-parity.test.ts plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts plugins/toolu/hooks/src/__tests__/post-tools-283.test.ts plugins/ast-grep/hooks/src/__tests__/golden-corpus.test.ts tools/toolu-conformance/src/harness/__tests__/posttool-corpus.test.ts",
    "model": "inherit"
  },
  {
    "id": "quality-cases",
    "title": "Export ts/python/rust-quality case data and move each golden capture to fixtures/quality with unchanged real project and edit steps",
    "ac_refs": ["AC-4", "AC-7"],
    "depends_on": ["inventory-loader", "conformance-corpora"],
    "paths": ["fixtures/quality/**", "plugins/ts-quality/hooks/src/**", "plugins/python-quality/hooks/src/**", "plugins/rust-quality/hooks/src/**", "tools/toolu-conformance/src/harness/**", "packages/toolu-core/src/**"],
    "input": "120 TS, 96 Python, and 119 Rust quality cases, including real write/delete/move and invalid/read-failure boundaries; existing golden outputs",
    "check": "capsh --drop=cap_dac_override,cap_dac_read_search -- -c 'bun test --timeout 60000 plugins/ts-quality/hooks/src/__tests__ plugins/python-quality/hooks/src/__tests__ plugins/rust-quality/hooks/src/__tests__'",
    "model": "inherit"
  },
  {
    "id": "ast-grep-runner",
    "title": "Export ast-grep nudge/savings/report and shared quality runner cases, including their existing goldens",
    "ac_refs": ["AC-4", "AC-7"],
    "depends_on": ["inventory-loader", "quality-cases"],
    "paths": ["fixtures/ast-grep/**", "fixtures/quality/runner.json", "plugins/ast-grep/hooks/src/**", "packages/toolu-core/src/quality/**", "tools/toolu-conformance/src/harness/**"],
    "input": "38 nudge, 35 savings, 6 report cases plus quality runner cases; structural-search advice, missing ast-grep, malformed ledger, and edit/delete paths",
    "check": "bun test --timeout 60000 plugins/ast-grep/hooks/src/__tests__ packages/toolu-core/src/quality/__tests__",
    "model": "inherit"
  },
  {
    "id": "future-port-cases",
    "title": "Record the inline baseline names and export host-root, state, statusline, OpenCode permission/evaluate and lifecycle event cases for their dependent Rust issues",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["inventory-loader", "existing-json"],
    "paths": ["fixtures/host/**", "fixtures/state/**", "fixtures/statusline/**", "fixtures/opencode/**", "packages/toolu-core/src/host/**", "packages/toolu-core/src/state/**", "plugins/statusline/hooks/src/**", "tools/toolu-opencode/src/**"],
    "input": "Current host-root and state edge cases, statusline states, and OpenCode permission/evaluate plus lifecycle event scenarios, including malformed/denied boundaries",
    "check": "bun test --timeout 60000 packages/toolu-core/src/host/__tests__ packages/toolu-core/src/state/__tests__ plugins/statusline/hooks/src/__tests__ tools/toolu-opencode/src",
    "model": "inherit"
  },
  {
    "id": "unbash-baseline",
    "title": "Record and verify the pinned unbash parse result for every distinct shell fixture input, including structured errors",
    "ac_refs": ["AC-6"],
    "depends_on": ["existing-json"],
    "paths": ["fixtures/shell/**", "packages/toolu-core/src/shell/__tests__/**", "tooling/src/check-unbash-baseline.ts", "tooling/src/__tests__/check-unbash-baseline.test.ts", "bun.lock"],
    "input": "Every distinct command in bats-parity.json and issue-283.json plus two malformed parser probes; pinned unbash parser's real output and parse-error shape",
    "check": "bun test --timeout 60000 tooling/src/__tests__/check-unbash-baseline.test.ts packages/toolu-core/src/shell/__tests__ && bun run tooling/src/check-unbash-baseline.ts",
    "model": "inherit"
  },
  {
    "id": "docs-gate",
    "title": "Document every fixture schema, consumer issue, baseline count and deferred tooling-only tree; audit JSON source-of-truth and run the full Bun gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["existing-json", "lifecycle-gates", "conformance-corpora", "quality-cases", "ast-grep-runner", "future-port-cases", "unbash-baseline"],
    "paths": [".", "fixtures/README.md", "docs/testing.md", "tooling/src/check-fixture-inventory.ts", ".github/ci-paths.json"],
    "input": "All committed JSON fixtures and former TypeScript case tables, real hook bundles and sandboxes, original and final case-name inventories, and the entire branch diff",
    "check": "bun run test && bun run tooling/src/check-fixture-inventory.ts && bun run check:ci-paths",
    "model": "inherit"
  }
]
```

## Critical files

Create `fixtures/index.json`, `fixtures/README.md`, case-family JSON and the pinned unbash baseline; add a strict JSON loader to `tools/toolu-conformance/src/harness/`, with an inventory and parser-baseline check under `tooling/src/`. Move the five existing fixture trees and golden captures. Update current TypeScript case consumers under `plugins/*/hooks/src/__tests__`, `packages/toolu-core/src/**/__tests__`, `tools/toolu-conformance/src/harness`, `tools/toolu-opencode/src`, and all exact path readers in docs/CI.

## Verification

Preserve every existing case name, count, golden output and assertion. Validate malformed JSON and descriptor boundaries against real temporary directories. Run each focused runner after its family moves, then the full `bun run test` gate under the epic job lease. Compare any root-host-only failure against a clean `origin/main` checkout; it still blocks delivery until resolved or CI proves the non-root result. After a scoped commit, final ledger `run <plan> --verify`, `toolu-review:review`, and `verdict.js status` must be ready before the final push; then verify the PR head/base, run babysit, and report ready only after its success stop.

## Plan review

- existing-json: 🟡 should-fix: a broad `test:unit` check would reach known root-host permission cases unrelated to moved readers. Replaced it with the focused shell/config/conformance CLI suites, gate inventory, and docs check; the final step still runs the complete gate. Jev favored this separation with 1.00 relative probability, and the verified root-host baseline supports it.
- paths: 🟡 should-fix: several steps omitted shared hook bundles and harness dependencies from freshness declarations. Added their read paths. Every AC-1 through AC-7 is mapped, every step has a runnable check, and the dependencies are acyclic.

## Deviations

- During `inventory-loader`, the JSON payload representation was specified more precisely: dynamic path and command-template values carry `$path` or `$template` tags. Plain strings remain literal so shell variables in existing cases cannot change meaning. This refines the approved spec without changing scope or step checks; Jev favored tagged values over global replacement (1.00 relative probability).
- The read-only inline-suite inventory showed that host/state/statusline/OpenCode cases are individual test declarations, not exported arrays. Their exact expanded names belong to `future-port-cases`, which owns their conversion; `inventory-loader` records the 13 existing exported arrays (979 names). Jev favored this dependency-aligned split (0.99 relative probability). AC coverage and checks are unchanged.
- The existing shell JSON has 201 distinct commands, none producing an unbash parse error. Added two separate malformed probes in `fixtures/shell/parser-errors.json`, bringing the pinned baseline to 203 distinct inputs without changing the legacy parity rows. The completeness check includes all three sources and rejects missing or extra inputs. Jev favored an explicit source fixture over baseline-only exceptions (1.00 relative probability).
- The two conformance corpora run each named case on Claude and Codex and have host-dependent setup paths. Their JSON records use `setupByHost` with one ordered action array per host rather than duplicating case rows. Jev favored this representation (0.96 relative probability); the original 33 and 16 names remain intact.
- The original `test:conformance` command checks four other conformance surfaces and does not replay the pre/post-tool corpora. The conformance-corpora ledger check now includes their current pre-tool golden consumers, the #283 post-tool cases, and a direct real-bundle replay of all 16 post-tool records on both hosts. Jev favored the expanded check (0.75 relative probability).
- This worker runs as root with DAC override, so the three unreadable-file tests fail despite otherwise matching goldens (708 pass). Each passes with `capsh --drop=cap_dac_override,cap_dac_read_search`; the local quality-cases ledger check uses that capability drop. CI continues to run plain Bun as a non-root user. Jev was unavailable (`no healthy upstream`), so the decision uses the direct before/after test evidence.
- The quality records need one symlink to the checkout’s real jscpd binary. Added `$REPO` only for tagged symlink targets inside this checkout; Jev favored it over a special action or copied binary (0.89 relative probability). Exported all 335 quality cases and their named deviations as JSON, using bounded `pathWithout`, `stubAstGrep`, and `projectSettings` environment descriptors.
