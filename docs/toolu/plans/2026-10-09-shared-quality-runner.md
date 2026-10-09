# Shared Rust quality runner — Plan

**Date:** 2026-10-09   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-09-shared-quality-runner-design.md   **Topic:** #459, one engine post-edit quality flow

## Evidence and approach

The approved spec fixes the behavior. `fixtures/quality/runner.json` is the TypeScript parity source. `toolu-state::edit_records`, `toolu-state::gate_file`, `toolu-runtime::process`, and `toolu-runtime::host::roots::Roots` already own normalization, locked state, subprocesses, and host state paths. `quality-edit.ts`, `quality-run.ts`, `quality-gate.ts`, and `quality-ast-grep.ts` specify the port's order and output. The current engine exposes `RuleContext` and compiles rule crates in through the hub; #426–#428 will supply `QualityRule` implementations. Rust's 300-line file, 50-line function, tests beside modules, 90% engine coverage, and no suppression apply.

The installed `toolu jev` has only `planned`, so semantic judgments use the issue, fixture, and source evidence. The initial design and plan live under ignored `docs/toolu/` and will be force-added intentionally.

## Workstream summary

Resolve edits and ownership → batch and parse structural scans → settle gate entries and prove moves/worktrees → document and run the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "quality-edit",
    "title": "Port edited-file extraction, delete/move fields, regular-file checks, linked-worktree detection, and extension ownership to toolu_engine::quality with shared JSON edit-case tests",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-engine quality_edit && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": ["AC-1", "AC-3"],
    "paths": ["crates/core/engine/src/quality/", "crates/core/engine/src/quality.rs", "crates/core/engine/src/lib.rs", "crates/core/engine/tests/quality_fixture.rs", "fixtures/quality/runner.json", "packages/toolu-core/src/quality/quality-edit.ts"],
    "input": "Every kind=edit check in fixtures/quality/runner.json in a real temp Git repo, including null/false/empty fallbacks, split override, symlink, directory, missing path, and a real linked worktree",
    "model": "inherit"
  },
  {
    "id": "quality-scan",
    "title": "Aggregate enabled rule-directory YAML, invoke one bounded ast-grep scan over the batch, parse flattened rule-tagged source lines, and distinguish missing/failed/invalid/empty output",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-engine quality_scan && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": ["AC-4", "AC-5"],
    "depends_on": ["quality-edit"],
    "paths": ["crates/core/engine/src/quality/", "crates/core/engine/src/quality.rs", "crates/core/engine/tests/quality_fixture.rs", "fixtures/quality/runner.json", "packages/toolu-core/src/quality/quality-ast-grep.ts"],
    "input": "Real ast-grep and its exported scan cases; two rule YAML directories and two source files; a counting executable that delegates to the real ast-grep; controlled missing, malformed, stderr, exit and signal failures",
    "model": "inherit"
  },
  {
    "id": "quality-runner",
    "title": "Implement QualityRule and the one-event runner with project checks, extension selection, source-owned gate settlement, move/delete handling, and per-rule linked-worktree policy; replay shared run cases",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-engine quality && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["quality-edit", "quality-scan"],
    "paths": ["crates/core/engine/src/quality/", "crates/core/engine/src/quality.rs", "crates/core/engine/src/lib.rs", "crates/core/engine/tests/quality_fixture.rs", "fixtures/quality/runner.json", "crates/core/state/src/gate_file.rs", "packages/toolu-core/src/quality/quality-run.ts", "packages/toolu-core/src/quality/quality-gate.ts"],
    "input": "Shared run JSON replayed against real state files, an apply_patch ts-to-py move from toolu-state, and git worktree add for the linked-worktree case; failures and advisories checked exactly",
    "model": "inherit"
  },
  {
    "id": "quality-docs-gate",
    "title": "Document the public QualityRule integration point in AGENTS.md and docs/registry.md, then run the full Rust and TypeScript gates on the final diff",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(engine): share post-edit quality runner' && if [ \"$(id -u)\" = 0 ]; then capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'bun run test'; else bun run test; fi",
    "ac_refs": ["AC-6"],
    "depends_on": ["quality-runner"],
    "paths": ["AGENTS.md", "docs/registry.md", "crates/core/engine/", "fixtures/quality/", "packages/toolu-core/src/quality/", "Cargo.toml", "Cargo.lock", "tooling/"],
    "input": "The complete branch diff and existing repository gates, with the shared JSON fixture and real Git integration tests",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/engine/src/quality.rs` and `quality/{edit,scan,run}.rs` with sibling `tests/*_test.rs`.
- `crates/core/engine/src/lib.rs` and `crates/core/engine/tests/quality_fixture.rs`.
- `AGENTS.md` and `docs/registry.md`.
- `docs/toolu/{brainstorms,specs,plans}/2026-10-09-shared-quality-runner*`, force-added because `docs/toolu/` is ignored.

## Verification

The Rust tests replay every runner fixture against real temp repositories and state files. A real apply_patch move and linked worktree prove the cross-rule state behavior. The scan tests use a real ast-grep binary and a delegating executable to count processes, plus controlled failures to prove propagation. `cargo xtask gate` and `bun run test` close the branch; release-readiness then runs ledger verify, local review, verdict, and a PR with babysit. Rebase on moving `origin/main` before implementation and push, and rerun affected checks.
