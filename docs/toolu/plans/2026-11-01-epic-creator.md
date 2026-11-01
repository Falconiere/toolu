# epic-creator plugin — Plan

**Date:** 2026-11-01   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-01-epic-creator-design.md   **Topic:** Markdown-only plugin that writes a GitHub epic the engine can run, with token-lean bodies

## Evidence and approach

Brainstorm record `docs/toolu/brainstorms/2026-11-01-epic-creator.md`. Template
for a Markdown-only plugin: `plugins/brainstorm` + `crates/brainstorm`; every
place brainstorm is registered was listed with `git grep` (marketplaces,
`Cargo.toml`, `crates/cli/Cargo.toml`, `registry.rs`, `tree_test.rs`,
`contract.rs` two lists, `helpers/schema.rs` guide list, catalog and order
tests, `context_budget/budgets.rs`, `release-please-config.json`,
`folders.json`, README, `docs/plugins/index.md`). Count constants from the
git-better addition (#764) and comemory 95a0ea5e: `packaging_assets.rs`
PLUGINS 16→17 and SKILLS 20→21, `helpers/inventory.rs` 16→17 with the test
name, `layers_check_test.rs` crates 30→31. The reader contract is
`tracker/github_fetch.rs::text_blockers` and `route/tier.rs::heuristic_tier`.

## Workstream summary

Reader extraction → body references and reader/tier tests → crate and
registration → skill, writes reference and skill test → brainstorm mode
wording → docs → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "reader-extract",
    "title": "Extract text_blocker_refs from text_blockers with no behavior change",
    "paths": ["crates/epic-orchestrator/src/tracker/github_fetch.rs", "crates/epic-orchestrator/src/tracker/tests/github_fetch_test.rs", "crates/epic-orchestrator/src/tracker/tests/github_fetch_reuse_test.rs"],
    "input": "existing github_fetch fixtures with blocked-by text",
    "check": "cargo test -p toolu-epic-orchestrator --lib tracker::",
    "model": "inherit"
  },
  {
    "id": "bodies",
    "title": "Issue body references with a filled example; reader and tier tests run the real code on it",
    "ac_refs": ["AC-1", "AC-2"],
    "depends_on": ["reader-extract"],
    "paths": ["plugins/epic-creator/skills/epic-creator/references/issue-bodies.md", "crates/epic-orchestrator/src/tracker/github_fetch.rs", "crates/epic-orchestrator/src/tracker/tests/epic_creator_bodies_test.rs", "crates/epic-orchestrator/src/route/tier.rs", "crates/epic-orchestrator/src/route/tests/epic_creator_tier_test.rs"],
    "input": "the shipped filled example sub-issue and epic template from issue-bodies.md; boundary: own reference and a prose issue mention",
    "check": "cargo test -p toolu-epic-orchestrator --lib epic_creator",
    "model": "inherit"
  },
  {
    "id": "crate-and-registration",
    "title": "Guide crate, manifests, marketplaces, CLI registry, counts and release config",
    "ac_refs": ["AC-3", "AC-4"],
    "paths": ["crates/epic-creator", "Cargo.toml", "Cargo.lock", "crates/cli/Cargo.toml", "crates/cli/src/registry.rs", "crates/cli/src/tests/tree_test.rs", "crates/cli/tests/contract.rs", "crates/cli/tests/helpers", "crates/cli/tests/fixtures/commands_json.snap", "crates/plugins/src/tests/catalog_test.rs", "crates/plugins/src/catalog/tests/order_test.rs", "crates/xtask/src/packaging_assets.rs", "crates/xtask/src/tests/layers_check_test.rs", "tooling/conventions/guardrails/rust/folders.json", "release-please-config.json", ".claude-plugin/marketplace.json", ".agents/plugins/marketplace.json", "plugins/epic-creator"],
    "input": "the repository's own manifests and the built toolu binary",
    "check": "cargo test -p toolu-epic-creator && cargo test -p toolu-cli --test contract && cargo test -p toolu-plugins catalog && cargo test -p xtask --bin xtask layers_check",
    "model": "inherit"
  },
  {
    "id": "skill",
    "title": "SKILL.md, github-writes reference, description budget and skill contract test",
    "ac_refs": ["AC-5", "AC-6"],
    "depends_on": ["bodies", "crate-and-registration"],
    "paths": ["plugins/epic-creator/skills/epic-creator", "crates/cli/tests/epic_creator_skill.rs", "crates/xtask/src/context_budget/budgets.rs", "tooling/conventions/markdown-cli.json"],
    "input": "the shipped SKILL.md and references",
    "check": "cargo test -p toolu-cli --test epic_creator_skill && cargo xtask context-budget && cargo xtask check-markdown-cli",
    "model": "inherit"
  },
  {
    "id": "brainstorm-mode",
    "title": "Generalize brainstorm's Delivery mode to any calling skill",
    "ac_refs": ["AC-7"],
    "paths": ["plugins/brainstorm/skills/brainstorm/SKILL.md", "crates/xtask/tests/skills_contract.rs"],
    "input": "brainstorm and delivery-flow SKILL.md",
    "check": "cargo test -p xtask --test skills_contract && cargo xtask context-budget",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "README, plugin index, epic-orchestrator pointer, regenerated CLI docs",
    "ac_refs": ["AC-3", "AC-4"],
    "depends_on": ["crate-and-registration", "skill"],
    "paths": ["README.md", "docs/plugins/index.md", "docs/epic-orchestrator/README.md", "docs/cli", "plugins/epic-creator/README.md"],
    "input": "built toolu binary for docs-cli",
    "check": "cargo xtask docs-cli --check && cargo xtask check-markdown-cli",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full local gate",
    "ac_refs": ["AC-8"],
    "depends_on": ["reader-extract", "bodies", "crate-and-registration", "skill", "brainstorm-mode", "docs"],
    "input": "the whole branch",
    "check": "DOCKER_CONTEXT=colima-toolu-gate cargo xtask deploy-gate",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/tracker/github_fetch.rs`, `route/tier.rs` and two new test files.
- `crates/epic-creator/**`, `plugins/epic-creator/**`.
- Registration files listed in `crate-and-registration`.
- `plugins/brainstorm/skills/brainstorm/SKILL.md`.

## Verification

End to end: the reader and router tests run the real crate code on the
shipped example body (AC-1, AC-2); the built binary prints the guide and
validates (AC-3); inventory, packaging and catalog tests pass on the real
manifests (AC-4); skill contract, context budget and markdown-cli pass
(AC-5, AC-6); skills_contract passes (AC-7); then the full gate (AC-8), then
`toolu ledger run <plan> --verify`. Live epic creation on GitHub is not run
(outward-facing); the reader contract is proven against real code instead.
Delivery: scoped commit, code review with v2 state, verdict ready, push, PR.
Per the user's instruction, no pr-babysit: rebase on main, resolve conflicts,
run code-review, fix all findings, commit, push and merge.

## Deviations

- `crate-and-registration`: the check used `--lib`; xtask has only a binary target, so it runs `--bin xtask`.
