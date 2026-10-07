# Pre-tool gates A in Rust — brainstorm

**Date:** 2026-10-07  **Issue:** #419  **Mode:** Delivery

## Outcome

`toolu hook pre-tools` makes the same protected-files, mcp-blocker, and code-edit-rules decisions as the TypeScript dispatcher, in the same module order, for the shared captures on Claude Code and Codex.

## Evidence and boundary

- `fixtures/gates/pre-tool-modules-a.json` has 109 cases; its golden capture also holds 22 pre-tool corpus rows. Some 109 cases use the separate `mcp-tools` entry. #422 owns that entry; this issue ports the mcp-blocker built-in used by `pre-tools`.
- `crates/core/engine/src/builtins.rs` has an empty pre-tool table. Its existing `Gate` trait and dispatch walk already encode decisions and apply precedence.
- `toolu-shell::writes::write_targets` identifies redirect and command write targets. `toolu-runtime` already resolves settings, config modes, and MCP blocklist entries.
- The TypeScript edit-rule gate tolerates extra, absent, and oddly typed JSON fields. The current Rust `code_edit_rules` settings loader rejects them, and shared runtime settings are outside this worker's scope.

## Decision

Implement the three gates and a shared Bash-style path matcher inside `toolu-engine`. Parse edit-rule JSON tolerantly in that gate. Use existing runtime functions for settings, config, and mode decisions, and shell analysis for write targets. Keep exported API additions to the gate table. Jev favored local tolerant parsing (confidence 1.0); its pattern-location choice was uncertain (0.37 for engine-local), so the repository's layer boundary and no-spawn hook budget decide that location.

## Risks and checks

The risk is pattern expansion and exact message drift, especially absolute paths, Bash pathname globs, invalid config, and Codex ask degradation. Replay all shared cases through `TOOLU_IMPL=rust:toolu/pre-tools`, add focused Rust tests for edge cases, and compare stdout, stderr, and exit code. Keep #421 ledger/resources and #445 management/config/status changes out of this branch.
