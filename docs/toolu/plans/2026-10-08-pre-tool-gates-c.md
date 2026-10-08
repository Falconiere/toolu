# Pre-tool gates C in Rust — Plan

**Date:** 2026-10-08   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-08-pre-tool-gates-c-design.md   **Topic:** #422: workflow gates, agent-tier and MCP hook in Rust.

## Evidence and approach

The approved spec fixes the five native behavior contracts and the 180 C fixture cases. #418 supplies the dispatcher, #421 supplies ledger, review, waiver and docs primitives, and #419–#420 supply six of nine ordered built-ins. The Rust quality bar requires colocated tests, no exemptions, 90% engine coverage and the full `cargo xtask gate`. Reuse existing primitives with hook-specific decisions; the standalone MCP test must use the `TOOLU_IMPL` launcher seam. This worktree's full checks use the epic job lease. `docs/toolu/` is ignored and its artifacts must be force-added.

## Workstream summary

Port shared push targeting and the three workflow gates, then add the two standalone native hooks. Replay the C fixtures and full A/B/C suite, update documentation, and run the full gates and delivery preflight.

## Steps (machine-readable)

```json
[
  {
    "id": "push-review",
    "title": "Port push-target and push-review with v2 state, waiver, modes and telemetry",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::push_review::tests::",
    "ac_refs": ["AC-1", "AC-5"],
    "paths": ["crates/core/engine/src/gates/", "crates/core/engine/src/builtins.rs", "crates/core/engine/src/review_state.rs", "crates/core/engine/src/waiver.rs", "crates/core/shell/src/", "crates/core/state/src/", "crates/core/runtime/src/"],
    "input": "Real git branches with a missing review, matching v2 review, stale SHA, empty diff, waiver and git -C push"
  },
  {
    "id": "plan-ledger-gate",
    "title": "Port the read-only plan-ledger push gate and optional AC coverage stop",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::plan_ledger::tests::",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["push-review"],
    "paths": ["crates/core/engine/src/gates/", "crates/core/engine/src/builtins.rs", "crates/core/engine/src/ledger/", "crates/core/runtime/src/", "crates/core/state/src/"],
    "input": "Real v1 ledger with fresh-green and stale steps, unparseable ledger, uncovered spec AC and whole-diff verification"
  },
  {
    "id": "docs-sync-gate",
    "title": "Port docs-sync path globs, attestation, config modes and telemetry",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::docs_sync::tests::",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["plan-ledger-gate"],
    "paths": ["crates/core/engine/src/gates/", "crates/core/engine/src/builtins.rs", "crates/core/engine/src/verdict/glob.rs", "crates/core/runtime/src/config/docs_sync.rs", "crates/core/state/src/"],
    "input": "Real branch with code-only diff, changed README, excluded release doc, current-SHA attestation and off/ask/block modes"
  },
  {
    "id": "agent-tier-hook",
    "title": "Route native agent-tier hook and preserve delegation telemetry and fail-open model advice",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-hub --lib agent_tier::tests:: && PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-cli --bin toolu hook::tests::",
    "ac_refs": ["AC-2", "AC-5"],
    "depends_on": ["docs-sync-gate"],
    "paths": ["crates/toolu/src/", "crates/cli/src/", "crates/core/engine/src/ledger/", "crates/core/state/src/telemetry.rs", "crates/core/runtime/src/"],
    "input": "Real ledger running step, next step, mismatched explicit model, omitted model, missing git and malformed payload"
  },
  {
    "id": "mcp-hook",
    "title": "Route native standalone MCP hook through mcp-blocker and adapt functional tests to the launcher seam",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-hub --lib mcp_hook::tests:: && PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-cli --bin toolu hook::tests::",
    "ac_refs": ["AC-3", "AC-5"],
    "depends_on": ["agent-tier-hook"],
    "paths": ["crates/toolu/src/", "crates/cli/src/", "crates/core/engine/src/gates/mcp_blocker.rs", "plugins/toolu/hooks/src/__tests__/mcp-tools.test.ts", "tools/toolu-conformance/src/harness/entry-command.ts"],
    "input": "Claude and Codex mcp__exampleblocked__search calls with real blocklist, unlisted server, malformed JSON and malformed config"
  },
  {
    "id": "fixture-parity",
    "title": "Replay all C and MCP cases, then the complete A/B/C pre-tool suite against the release binary",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo build --release -p toolu-cli && TOOLU_IMPL=rust:toolu/pre-tools,toolu/agent-tier,toolu/mcp-tools TOOLU_RUST_BIN_DIR=$PWD/target/release bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a.test.ts plugins/toolu/hooks/src/__tests__/pre-tool-modules-b.test.ts plugins/toolu/hooks/src/__tests__/pre-tool-modules-c.test.ts plugins/toolu/hooks/src/__tests__/mcp-tools.test.ts",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["mcp-hook"],
    "paths": ["crates/", "fixtures/gates/", "plugins/toolu/hooks/src/__tests__/", "tools/toolu-conformance/src/harness/"],
    "input": "All 180 C JSON cases, MCP host cases, A/B cases and the oversize shell case in fresh git sandboxes"
  },
  {
    "id": "docs",
    "title": "Document completed native built-ins and standalone hooks while retaining the #425 switch boundary",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo xtask check-markdown-cli && bun run test:docs",
    "ac_refs": ["AC-3", "AC-4"],
    "depends_on": ["fixture-parity"],
    "paths": ["docs/registry.md", "tools/toolu-opencode/generated/resources/repo/docs/registry.md", "docs/toolu/brainstorms/2026-10-08-pre-tool-gates-c.md", "docs/toolu/specs/2026-10-08-pre-tool-gates-c-design.md", "docs/toolu/plans/2026-10-08-pre-tool-gates-c.md"],
    "input": "Registry documentation checked against the actual native table and CLI hook commands"
  },
  {
    "id": "full-gate",
    "title": "Run the complete Rust and Bun quality gates on the final branch diff",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo xtask gate --base origin/main --title 'feat(engine): port pre-tool gates C to Rust (#422)' && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'PATH=/root/.cargo/bin:/root/.local/bin:$PATH bun run test'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["docs"],
    "paths": ["crates/", "Cargo.toml", "Cargo.lock", "fixtures/", "plugins/", "packages/", "docs/", "AGENTS.md", "tooling/"],
    "input": "The complete committed branch diff, native Rust integration tests and Bun conformance suites"
  }
]
```

## Critical files

`crates/core/engine/src/{builtins,gates}.rs` and their new colocated gate modules/tests; `crates/toolu/src/{agent_tier,mcp_hook,tool_hook}.rs`; `crates/cli/src/hook.rs`; the MCP conformance test; `docs/registry.md` and its generated OpenCode copy.

## Verification

The C fixture runner must compare parsed stdout, exact stderr/exit and touched files for non-deviation cases, not only decision kind. The MCP test must select the Rust binary through `launchedArgv`. Full A/B/C regression, Rust unit and integration tests, and the full Rust and Bun gates must pass. Rebase if `origin/main` moves and rerun affected checks.

## Delivery

After final checks, commit scoped files, run `toolu ledger run docs/toolu/plans/2026-10-08-pre-tool-gates-c.md --verify` on the whole diff, review the committed diff to a v2 zero-finding state, require `toolu ledger verdict status` to report `overall: ready`, push only `feat/422-pre-tool-gates-c-in`, open the PR against `main` with the brief's two issue lines first, verify head/base, and hand off to babysit. The orchestrator owns merge.
