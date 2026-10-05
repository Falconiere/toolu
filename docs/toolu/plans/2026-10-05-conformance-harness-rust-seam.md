# Conformance harness Rust seam — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-conformance-harness-rust-seam-design.md   **Topic:** #409: route black-box entries through one `TOOLU_IMPL` resolver

## Evidence and approach

The harness already centralizes subprocess behavior in `spawn.ts`; its `runHook`, `pretool.ts`, `posttool.ts` and `startup.ts` are the first route points. `protected-dispatch.ts` independently spawns the committed pre-tools bundle. The Rust workspace has no `toolu` CLI binary yet, so selected-command tests use real executable stubs and the committed CI manifest begins empty. Existing tests of epic `report.ts` and `epic-watch.ts` target legacy CLI/function behavior; the approved spec assigns their Rust replacement to #434/#435. Jev judged a central resolver preferable to per-caller selector logic (0.84) and later judged the epic-script replacement inventory preferable to invented legacy Rust verbs (0.99).

## Workstream summary

Resolver and boundary tests → hook harness and executable callers → CLI conformance → CI port list and workflow → documentation and full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "resolver",
    "title": "Add one exported entry-command resolver with strict TOOLU_IMPL selector parsing, default Bun argv, Rust hook argv, configured release binary path, and setup errors",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "paths": ["tools/toolu-conformance/src/harness/entry-command.ts", "tools/toolu-conformance/src/harness/__tests__/entry-command.test.ts", "tools/toolu-conformance/package.json", "plugins/toolu/hooks/dist/sample.js"],
    "input": "The committed sample bundle, a real executable shell stub in a temporary directory with spaces recording argv/stdin, and missing/non-file/non-executable target paths; selectors unset, rust, rust:toolu/pre-tools, a nonmatching entry, and malformed selector strings",
    "check": "bun test --timeout 60000 tools/toolu-conformance/src/harness/__tests__/entry-command.test.ts",
    "model": "inherit"
  },
  {
    "id": "callers",
    "title": "Route hook harness runners and all executable committed-bundle targets in black-box tests through the resolver, preserving existing default launcher argv and comparing stub results with the same golden expectations",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["resolver"],
    "paths": ["tools/toolu-conformance/src/harness/**", "plugins/**/hooks/src/__tests__/**", "packages/toolu-core/src/**/__tests__/**", "tools/toolu-opencode/src/**/__tests__/**"],
    "input": "The committed pre-tool golden corpus on Claude and Codex; a real allow-only toolu stub that records hook pre-tools stdin and causes deny-row comparison failures labelled Rust; existing bundle execution cases with TOOLU_IMPL unset",
    "check": "bun run test:unit",
    "model": "inherit"
  },
  {
    "id": "conformance",
    "title": "Route protected-dispatch through the resolver and keep all four CLI matrix suites active in both modes with Rust-labelled failures",
    "ac_refs": ["AC-1", "AC-3", "AC-4"],
    "depends_on": ["resolver"],
    "paths": ["tools/toolu-conformance/src/cli/**", "tools/toolu-conformance/src/harness/entry-command.ts", "tooling/fixtures/portable-core/protected-files-pre.json"],
    "input": "Real protected .env projects in normal and spaced cwd; selected executable stub returning deny; allow-only stub showing a Rust-labelled expectation failure; absent selector with the committed pre-tools bundle",
    "check": "bun test --timeout 60000 tools/toolu-conformance/src/cli/__tests__ && bun run test:conformance",
    "model": "inherit"
  },
  {
    "id": "ci",
    "title": "Add the sole Rust port manifest and a matrix job that no-ops when empty, otherwise builds the release CLI and runs test:unit plus test:conformance with the exact selector; keep path-group and aggregate contracts valid",
    "ac_refs": ["AC-3", "AC-5"],
    "depends_on": ["resolver", "conformance"],
    "paths": ["fixtures/rust-ported.json", "tooling/src/rust-conformance.ts", "tooling/src/__tests__/rust-conformance.test.ts", ".github/workflows/tests.yml", ".github/ci-paths.json", "package.json"],
    "input": "Committed empty manifest, malformed and duplicate temporary manifests, and a nonempty manifest that runs the real Cargo release build then fails because this skeleton workspace has no toolu CLI yet; the real workflow YAML and CI path data",
    "check": "bun test --timeout 60000 tooling/src/__tests__/rust-conformance.test.ts && bun run check:ci-paths && bun run tooling/src/rust-conformance.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document selector syntax, binary lookup, Rust port manifest, conformance scope, and the #434/#435 epic-script replacement inventory",
    "ac_refs": ["AC-1", "AC-5", "AC-6"],
    "depends_on": ["ci"],
    "paths": ["docs/testing.md", "docs/toolu/specs/2026-10-05-conformance-harness-rust-seam-design.md", "docs/toolu/brainstorms/2026-10-05-conformance-harness-rust-seam.md", "plugins/epic-orchestrator/scripts/__tests__/report.test.ts", "plugins/epic-orchestrator/scripts/__tests__/epic-watch.test.ts"],
    "input": "The real docs/testing.md, the two existing epic script test suites, and the approved replacement inventory in the spec",
    "check": "bun run test:docs && bun test --timeout 60000 plugins/epic-orchestrator/scripts/__tests__/report.test.ts plugins/epic-orchestrator/scripts/__tests__/epic-watch.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the full TypeScript gate, verify all ledger steps against the branch diff, review, and prepare the scoped conventional commit, PR and babysit handoff",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["callers", "conformance", "ci", "docs"],
    "paths": ["."],
    "input": "The entire branch, including the committed empty Rust manifest, golden corpus, real sandboxes and CI workflow",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

New: `tools/toolu-conformance/src/harness/entry-command.ts`, colocated tests, `fixtures/rust-ported.json`, `tooling/src/rust-conformance.ts` and its tests. Modified: harness runners and executable-target tests, `tools/toolu-conformance/src/cli/suites/protected-dispatch.ts`, `.github/workflows/tests.yml`, `.github/ci-paths.json`, `docs/testing.md`, and the conformance package exports. This plan and its approved spec are part of the branch.

## Verification

The test suite must prove the unchanged Bun default, exact Rust argv and stdin, selected missing-binary setup error, allow-only stub mismatch labelled Rust, and all four CLI suites executing with the selected hook path where applicable. The manifest runner must no-op when empty and reject malformed lists. The full `bun run test` gate and the CI path check must pass; repository-specific root-host failures, if any, require a clean `origin/main` comparison and still block delivery until resolved. After a scoped commit, run final ledger `run <plan> --verify`, `toolu-review:review`, and require `verdict.js status` to say `overall: ready`; then push, create or find the PR with the issue closing lines, and hand off to `pr-babysit:babysit`.

## Plan review

- callers: 🟡 should-fix: the step claimed migration of executable tests but checked only the harness and two golden files. Fixed by running `test:unit`, which reaches the changed black-box suites.
- ci: 🟡 should-fix: the step's positive stub case contradicted the current skeleton workspace, which has no `toolu` binary. Fixed by asserting the real missing-binary boundary for a nonempty manifest now; future ports supply the positive case.

AC-1…AC-6 each map to a runnable ledger step. Jev's relative ranking of the two gaps was diffuse (0.34 versus 0.31), so both were repaired against the concrete plan and repository state.
