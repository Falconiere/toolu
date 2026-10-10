# Shared Rust quality runner — Brainstorm

**Date:** 2026-10-09

## Outcome

One engine entry point handles a post-edit event for every enabled quality rule. It resolves edited paths, selects owners, runs structural rules in at most one ast-grep process, and settles each file's gate entry through `toolu-state`.

## Evidence and decisions

- `quality-edit.ts`, `quality-run.ts`, `quality-gate.ts`, and `quality-ast-grep.ts` define the existing semantics; `fixtures/quality/runner.json` exports their cases.
- `toolu-state::edit_records` already normalizes multi-file patches, and `toolu-state::gate_file` owns byte-compatible record and clear operations. The engine must reuse both.
- A rule describes its extension ownership, project preconditions, source/reason, linked-worktree policy, structural rule directories, and file check. The engine owns selection and state writes.
- TypeScript currently skips linked worktrees for ts-quality and checks them for python-quality and rust-quality. Preserve that per-rule policy; worktree state is rooted in the edited worktree.
- Collect structural YAML from the enabled, selected rules and scan all surviving edited files in one ast-grep call. Missing ast-grep and failed/invalid output remain distinct results for rule checks.

## Risk and alternatives

Running the old per-rule flow three times would duplicate file and gate handling and spawn several scans. A single `QualityRule` contract keeps that work in the engine. This is a public Rust interface: future rule crates must be able to call it without changing the module's state behavior. The immediate risk is preserving path, jq fallback, and process failure details across hosts and multi-file patches.

Jev could not judge the worktree ambiguity: the installed `toolu jev` only has the `planned` verb. The source modules and issue acceptance settle the choice above.
