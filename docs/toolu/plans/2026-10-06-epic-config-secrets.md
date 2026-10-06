# Epic engine config and secrets — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-epic-config-secrets-design.md   **Topic:** #463, shared settings and user-only secrets.

## Evidence and approach

#414 already delivered the open `epic` key in `config-schema.ts` and `config/load.rs`, with a shared golden. `Roots::config_root`, `Env`, `LoadedConfig` and the CLI's `Ctx`/`Outcome` are the reuse points. The epic plugin still has `Planned`, while `config get`, the expanded doctor, the journal and the status page belong to later issues. Comemory `d9a666f6` records the shared-runtime boundary and variable aliases. The spec fixes the file schema, failure behavior, redaction and CLI output. The root-host test caveat is comemory `be52369e`; CI and a main baseline distinguish it from a regression.

## Workstream summary

Typed non-secret settings → secure secret store and redaction → token CLI → docs and generated reference → full gates and delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "settings",
    "title": "Typed epic settings from the existing open namespace, with credential-key and peer URL rejection",
    "check": "cargo test -p toolu-runtime --lib config::epic::tests:: && cargo test -p toolu-runtime --test config_fixture && bun test packages/toolu-core/src/config/__tests__/config-fixture.test.ts",
    "ac_refs": ["AC-1", "AC-6"],
    "paths": ["crates/core/runtime/src/config.rs", "crates/core/runtime/src/config/", "crates/core/runtime/Cargo.toml", "Cargo.toml", "Cargo.lock", "packages/toolu-core/src/config/config-schema.ts", "packages/toolu-core/src/config/__tests__/config-fixture.test.ts", "fixtures/config/expected.json"],
    "input": "The existing shared config golden; real LoadedConfig data with default and valid epic sections, unknown safe keys, a sibling unknown top-level key, bad port and duplicate peers, and peer URLs with userinfo/path/query/fragment"
  },
  {
    "id": "secrets",
    "title": "No-follow 0600 file loader and atomic token rotation, environment precedence, redacted Debug and output serializers",
    "check": "cargo test -p toolu-runtime --lib config::secrets::tests::",
    "ac_refs": ["AC-2", "AC-3", "AC-5"],
    "depends_on": ["settings"],
    "paths": ["crates/core/runtime/src/config.rs", "crates/core/runtime/src/config/", "crates/core/runtime/src/host/roots.rs", "crates/core/runtime/src/env.rs", "crates/core/runtime/Cargo.toml", "Cargo.toml", "Cargo.lock"],
    "input": "Real temporary 0600 and 0644 files, symlinks, malformed JSON, env maps with primary/alias status and JSON peer overrides; canaries embedded in four representative serialized output documents; two atomic rotations with preserved fields"
  },
  {
    "id": "cli",
    "title": "toolu epic token new through the shared store, with no secret in either output mode; regenerate CLI reference and snapshot",
    "check": "cargo test -p toolu-epic-orchestrator && cargo test -p toolu-cli --test contract && cargo xtask docs-cli --check",
    "ac_refs": ["AC-4", "AC-5"],
    "depends_on": ["secrets"],
    "paths": ["crates/epic-orchestrator/", "crates/cli/", "crates/core/runtime/src/config/", "docs/cli/", "Cargo.toml", "Cargo.lock"],
    "input": "Real toolu binary invoked twice with --config-dir on a temp directory, then its 0600 secrets.json read and its stdout/stderr checked for both tokens; preexisting notification and peer fields; unsafe existing file; --json output"
  },
  {
    "id": "docs",
    "title": "Document the epic settings, secret schema, environment precedence, safe file mode, token command and future presenter contract",
    "check": "bun run test:docs && cargo xtask docs-cli --check",
    "ac_refs": ["AC-6", "AC-7"],
    "depends_on": ["cli"],
    "paths": ["docs/config.md", "docs/cli/", "docs/toolu/brainstorms/2026-10-06-epic-config-secrets.md", "docs/toolu/specs/2026-10-06-epic-config-secrets-design.md", "docs/toolu/plans/2026-10-06-epic-config-secrets.md", "AGENTS.md", "crates/core/runtime/src/config/", "crates/epic-orchestrator/src/"],
    "input": "User-facing config examples and generated toolu epic token new help; the source crate and command tree"
  },
  {
    "id": "gate",
    "title": "Full Rust and TypeScript quality gates with the shared parity tests and no new exemption",
    "check": "cargo xtask gate --base origin/main && bun run test",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["docs"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates/core/runtime/", "crates/epic-orchestrator/", "crates/cli/", "docs/config.md", "docs/cli/", "packages/toolu-core/src/config/", "fixtures/config/"],
    "input": "The entire branch; the Rust gate checks 90% runtime coverage, test layout, no secret, CLI drift and unused public items; Bun checks shared loader parity and docs, with any root-host-only failure baselined on origin/main"
  }
]
```

## Critical files

Create `crates/core/runtime/src/config/{epic,secrets}.rs` and their colocated `tests/*_test.rs`; extend `crates/core/runtime/src/config.rs` and crate dependencies. Add token subcommand to `crates/epic-orchestrator/src/lib.rs` with tests, update its planned-verb test and the CLI contract snapshot. Regenerate `docs/cli/` and update `docs/config.md`. Touch `AGENTS.md` only if the key-file description needs a new module mention.

## Verification

Use real filesystem modes and symlinks, actual OS randomness and process invocations. Red/green tests cover the invalid file and credential leak boundaries before implementing behavior. Run `cargo xtask gate --base origin/main` and `bun run test` under the epic job wrapper; compare any root-host-only Bun failure to a clean `origin/main` baseline, never waive an unexplained failure. The typed settings and CLI docs stay synchronized.

## Delivery

Keep scoped conventional commits and push this branch after affected tests. Run final ledger `run docs/toolu/plans/2026-10-06-epic-config-secrets.md --verify`, review with `toolu-review:review`, require `verdict.js status` `overall: ready`, fetch/rebase and recheck if main moved, then push and open a PR targeting `main` with the mandated issue/epic lines. Report `pr-open`, `babysit` and `ready` at their phase entries; hand off to `pr-babysit:babysit`. The orchestrator merges.

## Plan review

Jev rated step-to-AC alignment 1.65/2 (0.47 confidence) and flagged that the CLI step must invoke a real binary and compare both output modes against the generated token. `crates/cli/tests/contract.rs` already uses the real binary through `helpers/cli.rs`; its new test will read the temp secret file, run rotation twice and assert both token values are absent from stdout/stderr. The `cli` step covers that integration test. All AC-1 through AC-7 have ledger references, the file and environment failures are explicit, and docs and delivery checks are in order. No remaining blocker. **Status:** Approved.
