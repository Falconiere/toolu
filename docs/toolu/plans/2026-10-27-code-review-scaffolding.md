# code-review scaffolding — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-code-review-scaffolding-design.md   **Topic:** crate, namespace, contract, plugin assets and registration for `code-review` (#661)

## Evidence and approach

Inspected: `crates/brainstorm` and `crates/jev` (namespace and verb crate shapes), `crates/cli/src/registry.rs`, `crates/cli/tests/{contract.rs,helpers/inventory.rs,helpers/schema.rs}`, `crates/cli/src/commands.schema.json` (`$defs` per `--json` document), `crates/xtask/src/packaging_assets.rs`, `crates/toolu/src/doctor/tools.rs`, `toolu_protocol::exit::Exit`, `toolu_runtime::cli::{Ctx, Outcome}`, the plugin manifests and marketplaces, and `release-please-config.json`. Jev decided four things: no skill, no hook, a `run` verb, and fail-closed `unconfigured`/69.

Approach: build the crate first, with unit-tested contract, memory and budget types and doc-only stage modules. Next, wire the namespace into the CLI and its contract tests, with the `codeReview` schema `$def`. Then add the plugin assets and registration data. Generate the docs last, and finish with the full gate.

## Workstream summary

crate → plugin assets/registration (the CLI inventory test reads the manifests) → CLI registration + schema + black-box tests → generated docs + README → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "crate",
    "title": "crates/code-review: PLUGIN/command/run, cli, contract, memory, budget (Effort), doc-only stage modules, unit tests; workspace member and folders.json",
    "check": "test \"$(cargo test -p toolu-code-review -- --list 2>/dev/null | grep -c ': test$')\" -ge 8 && cargo test -p toolu-code-review",
    "paths": [
      "crates/code-review",
      "Cargo.toml",
      "Cargo.lock",
      "tooling/conventions/guardrails/rust/folders.json",
      "crates/cli/src/commands.schema.json"
    ],
    "ac_refs": [
      "AC-1"
    ],
    "input": "A hand-built Report with blocker, medium, low and nit findings (one side old, one without suggestion) validated against $defs/codeReview of crates/cli/src/commands.schema.json; every Effort value; memory records round-tripped through serde_json",
    "model": "inherit"
  },
  {
    "id": "assets",
    "title": "plugins/code-review README + 3 manifests, both marketplaces, release-please extra-files, packaging PLUGINS 13, doctor git row",
    "check": "cargo xtask packaging && cargo xtask check-hooks && cargo test -p toolu-hub doctor && cargo test -p xtask",
    "paths": [
      "plugins",
      ".claude-plugin/marketplace.json",
      ".agents/plugins/marketplace.json",
      "release-please-config.json",
      "crates/xtask",
      "crates/toolu/src/doctor"
    ],
    "depends_on": [
      "crate"
    ],
    "ac_refs": [
      "AC-5"
    ],
    "input": "The real plugins/ tree and marketplace files read by cargo xtask packaging",
    "model": "inherit"
  },
  {
    "id": "cli",
    "title": "Register code-review in crates/cli (registry, Cargo.toml, tree_test, contract order, inventory 13, schema routing) and add tests/code_review_cli.rs with help snapshot",
    "check": "cargo test -p toolu-cli --test code_review_cli && cargo test -p toolu-cli --test contract && cargo test -p toolu-cli --lib tree::tests::",
    "paths": [
      "crates/cli",
      "crates/code-review",
      "plugins",
      ".claude-plugin/marketplace.json",
      ".agents/plugins/marketplace.json"
    ],
    "depends_on": [
      "assets"
    ],
    "ac_refs": [
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5"
    ],
    "input": "The built toolu binary: `toolu --json code-review run --effort high` (exit 69, unconfigured document), `toolu code-review run --effort huge` (exit 64), `toolu code-review --help` (insta snapshot), `toolu commands --json` snapshot",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "cargo xtask docs-cli (docs/cli/code-review.md, commands.json), README count/table/tree, docs/plugins/index.md",
    "check": "cargo xtask docs-cli --check && cargo xtask check-cli-compat && cargo xtask check-markdown-cli && cargo xtask context-budget",
    "paths": [
      "docs",
      "README.md",
      "plugins/code-review",
      "crates/cli",
      "crates/code-review"
    ],
    "depends_on": [
      "cli"
    ],
    "ac_refs": [
      "AC-4",
      "AC-5"
    ],
    "input": "The built command tree rendered by docs-cli and compared with origin/main by check-cli-compat",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full local gate in the toolu-gate container",
    "check": "DOCKER_CONTEXT=colima-toolu-gate cargo xtask deploy-gate",
    "paths": [
      "."
    ],
    "depends_on": [
      "docs"
    ],
    "ac_refs": [
      "AC-5"
    ],
    "input": "The whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/code-review/{Cargo.toml,src/lib.rs,src/cli.rs,src/contract.rs,src/memory.rs,src/budget.rs,src/{shape,rules,prepass,review,validate,report}.rs,src/tests/*_test.rs,tests/schema.rs}`, `crates/cli/tests/code_review_cli.rs` plus its snapshot, and `plugins/code-review/{README.md,.claude-plugin/plugin.json,.codex-plugin/plugin.json,.muse-plugin/plugin.json}`.
- Modify: `Cargo.toml`, `Cargo.lock`, `crates/cli/Cargo.toml`, `crates/cli/src/registry.rs`, `crates/cli/src/tests/tree_test.rs`, `crates/cli/tests/contract.rs`, `crates/cli/tests/helpers/{inventory.rs,schema.rs}`, `crates/cli/tests/fixtures/commands_json.snap`, `crates/cli/src/commands.schema.json` and `docs/cli/commands.schema.json`, `crates/xtask/src/packaging_assets.rs`, `crates/toolu/src/doctor/tools.rs`, both marketplaces, `release-please-config.json`, `folders.json`, `README.md`, `docs/plugins/index.md`, and `docs/cli/*`.

## Verification

- End to end: the real binary prints a schema-valid `unconfigured` report and exits 69. A bad effort value exits 64. The help output matches its snapshot.
- Boundaries: the `medium` vs `low` blocking split, the old-side anchor, and an omitted suggestion.
- Docs: docs-cli output is current, compat stays additive, and the Markdown CLI check passes.
- Final step: `toolu ledger run <plan> --verify`, then the full gate.

## Plan review

- Order: should-fix, now fixed. The `contract` inventory test reads the plugin manifests, so `assets` now runs before `cli`.
- Paths: should-fix, now fixed. The `cli` step now declares `plugins` and both marketplaces, which the inventory test reads.
- Jev (executable and complete against the spec): 0.70. Every AC from AC-1 to AC-5 is mapped. Boundaries are covered: bad effort gives 64, `medium` vs `low` blocking, the old-side anchor and an omitted suggestion. Docs have their own step with `docs-cli --check`.
