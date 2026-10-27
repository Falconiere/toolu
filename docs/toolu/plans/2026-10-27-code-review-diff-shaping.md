# Code review diff shaping — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-code-review-diff-shaping-design.md   **Topic:** #662 native shape stage

## Evidence and approach

The approved spec follows #662 and #660. Reuse `toolu_state::git::base_branch`, `toolu_state::diff_sha::diff_sha`, `toolu_runtime::process`, and `contract::{Coverage, Side}`. Git supplies NUL-delimited metadata and per-entry patches. Rust classifies noise, builds old/new line anchors, and partitions the gate path set. The scaffold's `run` remains fail-closed until downstream stages land.

## Workstream summary

Build Git input and shape types, parse anchors, classify/partition/chunk, then verify against real repositories and synchronize internal docs.

## Steps (machine-readable)

```json
[
  {
    "id": "base",
    "title": "Resolve exact committed base and enumerate paths",
    "ac_refs": ["AC-1"],
    "paths": ["crates/code-review/Cargo.toml", "crates/code-review/src/shape.rs", "crates/code-review/src/shape/git_io.rs", "crates/code-review/src/shape/parse.rs", "crates/code-review/src/shape/file.rs", "crates/code-review/src/shape/noise.rs", "crates/code-review/src/shape/source.rs", "crates/code-review/src/budget.rs", "crates/code-review/src/tests/shape_test.rs", "crates/core/state/src/diff_sha.rs", "crates/core/state/src/git.rs"],
    "input": "Real Git repositories with diverged, unrelated, and empty branch histories",
    "check": "cargo test -p toolu-code-review shape_base"
  },
  {
    "id": "anchors",
    "title": "Shape renames and old/new changed-line anchors",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["base"],
    "paths": ["crates/code-review/src/shape.rs", "crates/code-review/src/shape/parse.rs", "crates/code-review/src/shape/file.rs", "crates/code-review/src/shape/tests/parse_test.rs", "crates/code-review/src/shape/tests/file_test.rs", "crates/code-review/src/tests/shape_test.rs", "crates/code-review/src/tests/shape_edge_test.rs", "crates/code-review/tests/fixtures/shape/rename.json", "crates/code-review/tests/fixtures/shape/deleted.json"],
    "input": "Committed git mv with edits and a committed deletion; golden shaped JSON",
    "check": "cargo test -p toolu-code-review shape_anchor"
  },
  {
    "id": "coverage",
    "title": "Classify noise, partition every path, and chunk whole files",
    "ac_refs": ["AC-3", "AC-5"],
    "depends_on": ["anchors"],
    "paths": ["crates/code-review/src/shape.rs", "crates/code-review/src/shape/parse.rs", "crates/code-review/src/shape/file.rs", "crates/code-review/src/shape/noise.rs", "crates/code-review/src/shape/source.rs", "crates/code-review/src/shape/tests/noise_test.rs", "crates/code-review/src/shape/tests/source_test.rs", "crates/code-review/src/budget.rs", "crates/code-review/src/tests/shape_test.rs", "crates/code-review/tests/fixtures/shape/noise.json"],
    "input": "Committed lockfile, vendor, build, generated attr and marker, binary, plus an oversized file; golden shaped JSON",
    "check": "cargo test -p toolu-code-review shape_coverage"
  },
  {
    "id": "docs-gate",
    "title": "Synchronize documentation and pass the complete gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["coverage"],
    "paths": ["crates/cli/src/commands.schema.json", "docs/cli/commands.schema.json", "crates/code-review/Cargo.toml", "crates/code-review/src/shape.rs", "crates/code-review/src/shape/git_io.rs", "crates/code-review/src/shape/parse.rs", "crates/code-review/src/shape/file.rs", "crates/code-review/src/shape/noise.rs", "crates/code-review/src/shape/source.rs", "crates/code-review/src/shape/tests/file_test.rs", "crates/code-review/src/shape/tests/git_io_test.rs", "crates/code-review/src/shape/tests/noise_test.rs", "crates/code-review/src/shape/tests/parse_test.rs", "crates/code-review/src/shape/tests/source_test.rs", "crates/code-review/src/budget.rs", "crates/code-review/src/tests/shape_test.rs", "crates/code-review/src/tests/shape_edge_test.rs", "crates/code-review/tests/shape_contract.rs", "crates/code-review/tests/fixtures/shape", "docs/toolu/brainstorms/2026-10-27-code-review-diff-shaping.md", "docs/toolu/specs/2026-10-27-code-review-diff-shaping-design.md", "docs/toolu/plans/2026-10-27-code-review-diff-shaping.md"],
    "input": "The branch diff and the real Git fixtures from the preceding steps",
    "check": "cargo xtask gate"
  }
]
```

## Critical files

`crates/code-review/Cargo.toml`, `src/shape.rs`, `src/shape/{git_io,parse,file,noise,source}.rs`, their sibling tests, `src/budget.rs`, `src/tests/shape_test.rs`, `tests/shape_contract.rs`, `tests/fixtures/shape/*.json`, and this design/plan pair.

## Deviations

File inspection and noise policy were split into submodules to satisfy the repository's function and file-size gate. Their colocated tests and one black-box public-contract test satisfy the colocated and unused-public-item gates. Their paths are declared in the relevant ledger steps.

The full workspace gate exposed an existing mismatch between the shipped epic status JSON and the CLI schema. The schema and generated copy were updated to describe the already documented output; the focused CLI schema test passed. No epic status behavior changed.

The macOS full gate reaches launcher tests that intentionally use the installed fixed-path binary. The complete gate for this worktree runs in the existing Linux gate image under the epic job lease; the image tests the exact staged patch as a temporary commit.

Local xhigh review found source text that could be misclassified as binary or generated, wildcard pathspec expansion, and a moving diff hazard. The reviewed fixes keep those paths in coverage and reject mixed evidence. It also found three Git processes per file; a real 500-file fixture took 29.1 seconds. Batched committed attributes and generator-header checks brought the same fixture to 7.6 seconds without changing the golden shaped outputs. Other merge-base errors now remain Git errors, and the Git tests isolate global configuration.

A second xhigh review found a branch-controlled `linguist-generated` skip, broad build and marker rules, rename coverage under `diff.renames=false`, and CRLF anchor loss. The attributes now come from the merge-base, uncertain source stays reviewable, gate-listed rename aliases share coverage, and changed-line text keeps committed carriage returns. Mode-only and empty-file changes remain unreviewed by the approved no-anchor rule.

## Verification

Focused real Git tests prove the five ACs and compare stable shaped output to golden fixtures. They also prove an added empty file is explicitly unreviewed. `cargo xtask gate` verifies repository guardrails. Before any implementation run `toolu ledger preflight` on this plan. After the implementation, make a scoped commit and run `toolu ledger run` with `--verify`; locally run `claude -p --dangerously-skip-permissions "/code-review xhigh --fix"`, fix its findings, and rerun the gate. Rebase on current `origin/main`, rerun review and gate if it moved, push, verify the PR head/base and resolved threads, then merge as the user directed.
