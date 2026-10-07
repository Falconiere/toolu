# Pre-tool gates A in Rust — Plan

**Date:** 2026-10-07  **Status:** Approved  **Spec:** docs/toolu/specs/2026-10-07-pre-tool-gates-a-design.md  **Topic:** #419 native protected-files, mcp-blocker, and code-edit-rules

## Evidence and approach

The reviewed spec sets the #419/#422 boundary. `toolu-engine` already owns the `Gate` trait, dispatch walk, and empty pre-tool built-in table. Runtime supplies settings precedence, blocklist parsing, config modes, and guardrail decisions; shell supplies ordered write targets. The TypeScript source and 109 shared cases define messages and lenient edit-rule behavior. Comemory `4a085d57` and `f8e84ece` explain admission refusals and the doubled `--` for epic jobs. Jev favored local tolerant JSON parsing and an engine-local matcher, and classified the standalone MCP hook as #422 work.

## Workstream summary

Context plumbing and pattern semantics → three gates in table order → shared capture and portable-core replay → coverage inventory and docs → full gates and delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "context-patterns",
    "title": "Carry the explicit plugin root to built-ins and add a Bash-style path matcher plus pathname expansion, with real-directory tests",
    "check": "cargo test -p toolu-engine --lib gates::pattern::tests && cargo test -p toolu-engine --lib gates::gate_paths::tests",
    "ac_refs": ["AC-2", "AC-4"],
    "paths": ["crates/core/runtime/src/registry/rule.rs", "crates/core/engine/src/dispatch/event.rs", "crates/core/engine/src/gates/pattern.rs", "crates/core/engine/src/gates/gate_paths.rs", "crates/core/engine/src/gates/tests/pattern_test.rs", "crates/core/engine/src/gates/tests/gate_paths_test.rs"],
    "input": "real temporary directories with .env and .en[v], absolute and relative targets, **/ and extglob patterns, unmatched literal fallback"
  },
  {
    "id": "protected-files",
    "title": "Port protected-files over edit paths and toolu-shell write targets, preserving candidate and pattern order and exact mode messages",
    "check": "cargo test -p toolu-engine --lib gates::protected_files::tests",
    "ac_refs": ["AC-2", "AC-6"],
    "depends_on": ["context-patterns"],
    "paths": ["crates/core/engine/src/gates/protected_files.rs", "crates/core/engine/src/gates/tests/protected_files_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/shell/src/writes.rs", "fixtures/portable-core/protected-files-pre.json"],
    "input": "Edit of .env, Bash echo SECRET=1 >.env, cp -t, dynamic write, and protected .git path under real temporary repositories"
  },
  {
    "id": "mcp-blocker",
    "title": "Port mcp-blocker with blocklist precedence, disabled config entries even under an invalid envelope, and exact guardrail messages",
    "check": "cargo test -p toolu-engine --lib gates::mcp_blocker::tests",
    "ac_refs": ["AC-3"],
    "depends_on": ["context-patterns"],
    "paths": ["crates/core/engine/src/gates/mcp_blocker.rs", "crates/core/engine/src/gates/tests/mcp_blocker_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/runtime/src/config/settings.rs"],
    "input": "mcp__exampleblocked__search with a real blocklist file and redirect, invalid user config with mcp server false, and unlisted server"
  },
  {
    "id": "code-edit-rules",
    "title": "Port tolerant code-edit-rules JSON reading and first-match advice, including extra docs and malformed input",
    "check": "cargo test -p toolu-engine --lib gates::code_edit_rules::tests",
    "ac_refs": ["AC-4"],
    "depends_on": ["context-patterns"],
    "paths": ["crates/core/engine/src/gates/code_edit_rules.rs", "crates/core/engine/src/gates/tests/code_edit_rules_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "packages/toolu-core/src/gates/code-edit-rules.ts"],
    "input": "real code-edit-rules.json files with first match, extra docs, missing docs, extra keys, malformed JSON and non-string values"
  },
  {
    "id": "hook-parity",
    "title": "Replay the 109-case A fixture, 22 selected corpus captures, portable-core fixture, and direct TypeScript/Rust message pairs through the hook seam",
    "check": "cargo build --release -p toolu-cli && TOOLU_IMPL=rust:toolu/pre-tools bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a.test.ts plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts && cargo test -p toolu-cli --test pre_tool_gates",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-6"],
    "depends_on": ["protected-files", "mcp-blocker", "code-edit-rules"],
    "paths": ["plugins/toolu/hooks/src/__tests__/pre-tool-modules-a.test.ts", "plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts", "plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-cases.ts", "crates/cli/tests/pre_tool_gates.rs", "fixtures/gates/pre-tool-modules-a.json", "fixtures/gates/pre-tool-modules-a-golden.json", "fixtures/portable-core/protected-files-pre.json"],
    "input": "all committed fixture payloads, each in a real git sandbox; the paired test forces TOOLU_IMPL empty for its TypeScript run; CLI integration converts the portable-core Edit event to a host payload"
  },
  {
    "id": "inventory-docs",
    "title": "Record #419 Rust coverage for exactly three inventory rows and document the engine's new pre-tool built-ins",
    "check": "bun run tooling/src/gate-coverage-inventory.ts check && bun run test:docs",
    "ac_refs": ["AC-5"],
    "depends_on": ["hook-parity"],
    "paths": ["fixtures/gate-coverage/inventory.json", "docs/registry.md", "docs/gate-coverage-matrix.md", "docs/toolu/specs/2026-10-07-pre-tool-gates-a-design.md"],
    "input": "the three existing builtin-module PreToolUse inventory rows; current hooks.json still routes Bun until #425"
  },
  {
    "id": "full-gates",
    "title": "Run the repository's full Rust and TypeScript quality gates with no exemption",
    "check": "cargo xtask gate --base origin/main && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'bun run test'",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["inventory-docs"],
    "paths": ["crates/core/engine/src/", "crates/core/runtime/src/registry/rule.rs", "crates/cli/tests/pre_tool_gates.rs", "plugins/toolu/hooks/src/__tests__/pre-tool-modules-a.test.ts", "fixtures/gate-coverage/inventory.json", "docs/registry.md"],
    "input": "full workspace against the native hook implementation and real fixture suites"
  }
]
```

## Critical files

`crates/core/engine/src/{builtins,gates}.rs`, `crates/core/engine/src/gates/`, `crates/core/runtime/src/registry/rule.rs`, `crates/core/engine/src/dispatch/event.rs`, `crates/cli/tests/pre_tool_gates.rs`, the A fixture test, `fixtures/gate-coverage/inventory.json`, and `docs/registry.md`.

## Deviations

The path matcher and pathname expansion landed in separate engine files to keep each under the Rust file limit. The `context-patterns` check and paths now cover both modules.

On this root host, the Bun gate runs with DAC and ptrace capabilities dropped so unreadable-file and `/proc` permission fixtures exercise their intended behavior. The `full-gates` check records that environment explicitly.

## Verification

Use `PATH=/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH` for validation through the epic job lease. First capture a failing native fixture case while the table is empty; then run each focused check, the full A and portable-core replay, the direct message comparison, `cargo xtask gate --base origin/main`, and `bun run test`. Confirm no new gate data or suppressions. After the full gate, verify the final ledger against the branch diff, run the pre-push review and verdict, commit and push this scoped branch, then open the PR and hand off to babysit. Rebase and rerun affected checks if `origin/main` advances before implementation or push.
