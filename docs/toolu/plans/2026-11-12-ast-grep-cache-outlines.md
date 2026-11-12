# Content-addressed outlines and repository map — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-ast-grep-cache-outlines-design.md   **Topic:** Cache outlines and summarize large directories

## Evidence and approach
Merged #857 supplies `outline`, its batched scan runner and map. `toolu_state::git::common_dir`, `toolu_runtime::atomic::write_atomic`, and `retention::sweep_dir` establish the storage and cleanup patterns. Preserve the small map and quality scan behavior.

## Workstream summary
Add content IDs and cache with real Git tests, then large and outside-Git maps, then retention and documentation. Run focused checks after each step and the full gate before delivery.

## Steps (machine-readable)
```json
[
  {
    "id": "content-cache",
    "title": "Hash working files and cache path-free outlines",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["crates/core/state/src/content_id.rs", "crates/core/state/src/lib.rs", "crates/core/state/src/tests/content_id_test.rs", "crates/core/engine/src/outline.rs", "crates/core/engine/src/outline/cache.rs", "crates/core/engine/src/outline/tests/cache_test.rs", "crates/core/engine/Cargo.toml", "Cargo.lock", "crates/cli/tests/ast_grep_outline.rs"],
    "input": "A real temporary Git repository, dirty and untracked Rust files, a linked worktree, and truncated cache JSON",
    "check": "cargo test -p toolu-state --lib content_id && cargo test -p toolu-engine --lib outline::cache && cargo test -p toolu-cli --test ast_grep_outline"
  },
  {
    "id": "large-map",
    "title": "Summarize large Git maps and walk outside-Git directories",
    "depends_on": ["content-cache"],
    "ac_refs": ["AC-4", "AC-5"],
    "paths": ["crates/ast-grep/src/map.rs", "crates/ast-grep/src/map/large.rs", "crates/ast-grep/src/tests/map_test.rs", "crates/cli/tests/ast_grep_outline.rs"],
    "input": "160 real files across directories plus a non-Git directory",
    "check": "cargo test -p toolu-cli --test ast_grep_outline"
  },
  {
    "id": "retention-docs",
    "title": "Sweep outline entries at SessionStart and document behavior",
    "depends_on": ["content-cache", "large-map"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/ast-grep/src/retention.rs", "crates/ast-grep/src/retention/outline.rs", "crates/ast-grep/src/register.rs", "crates/ast-grep/src/tests/retention_test.rs", "docs/ast-grep/README.md"],
    "input": "Real cache files with pinned mtimes around 14 days and a reduced test capacity; production uses 20,000",
    "check": "cargo test -p toolu-ast-grep retention && cargo xtask check-markdown-cli"
  },
  {
    "id": "final-gate",
    "title": "Verify the complete branch against the quality gate",
    "depends_on": ["content-cache", "large-map", "retention-docs"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "paths": ["crates", "docs/ast-grep/README.md", "docs/toolu/specs/2026-11-12-ast-grep-cache-outlines-design.md", "docs/toolu/plans/2026-11-12-ast-grep-cache-outlines.md"],
    "input": "The committed branch diff and the complete Rust workspace",
    "check": "cargo xtask gate"
  }
]
```

## Critical files
The paths in the three ledger steps, plus this plan and its spec.

## Deviations
The large-map and retention code needed `map/large.rs` and `retention/outline.rs` to stay within the repository's 300-code-line and 50-line-function limits. Their paths are now declared in the ledger. The content-cache check also runs the real working-tree blob ID test.

## Verification
Run real Git and ast-grep tests, quality scan regression tests, and `toolu epic job -- cargo xtask gate`. Before editing, `toolu ledger preflight` must pass. Before push, commit a scoped conventional change, run `toolu ledger run docs/toolu/plans/2026-11-12-ast-grep-cache-outlines.md --verify`, complete native code review and ready verdict, and verify the PR head/base. Delivery authorization comes from the worker brief and delivery-flow.
