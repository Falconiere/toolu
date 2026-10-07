# Pre-tool gates B in Rust — Plan

**Date:** 2026-10-07   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-07-pre-tool-gates-b-design.md   **Topic:** #420: bash-commands, commit-gate, and quality-gate in `toolu-engine`.

## Evidence and approach

The spec pins TypeScript's built-in order and the 110 committed B fixtures. #418 supplies the Rust `Gate` walk; #419 and #423 supply working sibling gates and sandbox tests. `toolu-shell` supplies per-simple-command matching and parsed git invocations; `toolu-runtime` supplies settings and modes; `toolu-state` classifies gate files and knows git worktrees. The `TOOLU_IMPL` seam selects the Rust `toolu hook pre-tools` binary. The design permits legacy parseable gate state because the TypeScript reader and committed fixtures do.

The Rust quality bar requires colocated unit tests, ≤300 code lines per source file, ≤50 per function, no suppression, 90% engine coverage, and a full `cargo xtask gate`. `docs/toolu/` is ignored and must be force-added. The command shell needs `/root/.cargo/bin` and `/root/.local/bin` on `PATH`. Full checks use the epic job lease; when admission is refused, wait and retry the same check.

## Workstream summary

Port the three gates in existing order, prove fixture parity and cross-runtime state interleaving, update the native-engine documentation, then run the complete Rust and Bun gates.

## Steps (machine-readable)

```json
[
  {
    "id": "bash-commands",
    "title": "Port the allow/deny lists and per-simple-command shell analysis, including unknown input and host-specific gate modes",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::bash_commands::tests::",
    "ac_refs": ["AC-2", "AC-4"],
    "paths": ["crates/core/engine/src/gates/bash_commands.rs", "crates/core/engine/src/gates/tests/bash_commands_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/shell/src/", "crates/core/runtime/src/config/"],
    "input": "Real shell analyses of cd /tmp && node -e 1, an allowed node -e beside a denied one, a heredoc that only contains a rule, and an oversize command"
  },
  {
    "id": "commit-gate",
    "title": "Port static Conventional Commit prefix checks and the pre-commit reminder over parsed git invocations",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::commit_gate::tests::",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["bash-commands"],
    "paths": ["crates/core/engine/src/gates/commit_gate.rs", "crates/core/engine/src/gates/tests/commit_gate_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/shell/src/", "crates/core/state/src/git.rs"],
    "input": "Real git repositories with origin/HEAD, valid and invalid -m subjects, a dynamic first message, and git commit behind sudo or bash -c"
  },
  {
    "id": "quality-gate",
    "title": "Port failing gate-state enforcement for commit/push with state-crate reads, legacy values, and linked-worktree behavior",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --lib gates::quality_gate::tests::",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["commit-gate"],
    "paths": ["crates/core/engine/src/gates/quality_gate.rs", "crates/core/engine/src/gates/tests/quality_gate_test.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/state/src/gate_file.rs", "crates/core/state/src/git.rs", "crates/core/runtime/src/json/"],
    "input": "A real gate file under a git project: failing strict and legacy JSON, malformed JSON, object/array reason values, a linked worktree, and git commit/push behind wrappers"
  },
  {
    "id": "fixture-parity",
    "title": "Run the committed B cases and #283 cases through the native binary via the existing TOOLU_IMPL conformance seam",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo build --release -p toolu-cli && TOOLU_IMPL=rust:toolu/pre-tools TOOLU_RUST_BIN_DIR=$PWD/target/release bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-b.test.ts",
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "depends_on": ["quality-gate"],
    "paths": ["crates/core/engine/src/", "crates/toolu/src/", "crates/cli/src/", "crates/core/engine/Cargo.toml", "fixtures/gates/pre-tool-modules-b.json", "fixtures/gates/pre-tool-modules-b-golden.json", "plugins/toolu/hooks/src/__tests__/pre-tool-modules-b.test.ts", "plugins/toolu/hooks/src/__tests__/pre-tool-modules-b-cases.ts", "tools/toolu-conformance/src/harness/entry-command.ts"],
    "input": "All 110 sandboxed JSON cases plus the test's three oversize parser cases, with Bash/Shell tools on Claude and Codex and the committed TypeScript golden"
  },
  {
    "id": "state-interleaving",
    "title": "Prove TypeScript and Rust post-tool failures and clears are consumed by the other runtime's pre-tool quality gate",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo test -p toolu-engine --test pre_gates_interleave",
    "ac_refs": ["AC-3"],
    "depends_on": ["fixture-parity"],
    "paths": ["crates/core/engine/tests/pre_gates_interleave.rs", "crates/core/engine/tests/helpers/", "crates/core/engine/src/gates/", "crates/core/state/src/", "plugins/toolu/hooks/dist/pre-tools.js", "plugins/toolu/hooks/dist/post-tools.js"],
    "input": "One real git sandbox and its quality-gate-status.json; Bun and Rust post-tool quality-command failures followed by the other runtime's pre-tool commit check, then a passing quality command"
  },
  {
    "id": "docs",
    "title": "Document the three native built-ins and the #425 host-switch boundary in the registry guide",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo xtask check-markdown-cli && bun run test:docs",
    "ac_refs": ["AC-1", "AC-4"],
    "depends_on": ["state-interleaving"],
    "paths": ["docs/registry.md", "docs/toolu/brainstorms/2026-10-07-pre-tool-gates-b.md", "docs/toolu/specs/2026-10-07-pre-tool-gates-b-design.md", "docs/toolu/plans/2026-10-07-pre-tool-gates-b.md"],
    "input": "The documented current native-engine built-in table and the existing Bun host entry"
  },
  {
    "id": "full-gate",
    "title": "Run the complete Rust and Bun quality gates on the final branch diff",
    "check": "PATH=/root/.cargo/bin:/root/.local/bin:$PATH cargo xtask gate --base origin/main --title 'feat(engine): port pre-tool gates B to Rust (#420)' && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'PATH=/root/.cargo/bin:/root/.local/bin:$PATH bun run test'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["docs"],
    "paths": ["crates/", "Cargo.toml", "Cargo.lock", "fixtures/", "plugins/", "packages/", "docs/", "AGENTS.md", "tooling/"],
    "input": "The whole committed branch diff, its real Rust integration tests and Bun conformance suites"
  }
]
```

## Critical files

- `crates/core/engine/src/{builtins,gates}.rs`, `gates/{bash_commands,commit_gate,quality_gate}.rs`, and their colocated `gates/tests/*_test.rs`.
- `crates/core/engine/tests/pre_gates_interleave.rs`; existing sandbox helpers and the committed B fixture runner.
- `docs/registry.md` and the force-added design artifacts under `docs/toolu/`.

## Verification

The fixture seam must pass all named outcomes and exact non-deviation goldens on the Rust binary. The interleaving test must show both write/read directions and a clear. Each module's unit tests must cover allow and deny, modes, malformed/absent inputs, and wrapper boundaries. Documentation must state which path is native today. Run `cargo xtask gate` with the PR title and `bun run test` through the epic lease before delivery.

## Delivery

After all steps are green, fetch and rebase if `origin/main` moved, then re-run affected checks. Commit scoped files and run `toolu ledger run docs/toolu/plans/2026-10-07-pre-tool-gates-b.md --verify` against the final branch diff. Review that committed diff with `toolu-review:review` until its v2 state covers every changed file with zero findings; require `toolu ledger verdict status` to report `overall: ready`. Confirm GitHub authentication, feature branch, and babysit skill, push only `feat/420-pre-tool-gates-b-in`, open a PR against `main` with `Closes Falconiere/toolu#420` and `Part of Falconiere/toolu#402` first in its body, verify the PR head/base, and hand it to `pr-babysit:babysit`. The orchestrator owns merge.
