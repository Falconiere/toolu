# Code review deterministic pre-pass — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-code-review-deterministic-pre-pass-design.md   **Topic:** #664 trusted scanners before the LLM

## Evidence and approach

Approved spec for #664. Reuse `shape::Shaped` / `ChangedLine`, `contract::{Finding, Source, Severity, Side}`, `budget` constants, `toolu_engine::quality::{scan_inline, AstGrepScan, EditedFile}`, `toolu_runtime::process` for `git cat-file`, and the shared secrets table via `include_str!` of `tooling/conventions/guardrails/rust/rules.json`. Plugin crate may depend on engine; nothing depends on xtask. Jev: engine `scan_inline` (0.97), `include_str` secrets (0.71), optional `redaction_spans` (0.83), findings+notices Mechanical checks (0.92), New-side only, prefer-secrets cap.

## Workstream summary

Contract/budget extensions → secrets+entropy → ast-grep materialize+scan → cap/mechanical helpers → focused tests → schema + gate.

## Steps (machine-readable)

```json
[
  {
    "id": "contract",
    "title": "Add redaction_spans on Finding, PREPASS_FINDINGS_MAX, PrepassResult types, schema field",
    "ac_refs": ["AC-7"],
    "paths": [
      "crates/code-review/src/contract.rs",
      "crates/code-review/src/budget.rs",
      "crates/code-review/src/prepass.rs",
      "crates/code-review/src/prepass/mod.rs",
      "crates/code-review/src/tests/contract_test.rs",
      "crates/cli/src/commands.schema.json",
      "crates/cli/tests/helpers/code_review_schema.rs",
      "crates/code-review/Cargo.toml"
    ],
    "input": "Existing Finding JSON fixtures plus an optional redaction_spans array; empty spans omitted",
    "check": "cargo test -p toolu-code-review contract_ && cargo test -p toolu-cli code_review"
  },
  {
    "id": "secrets",
    "title": "Native secrets+entropy scanner over shaped New lines with masked quotes and spans",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["contract"],
    "paths": [
      "crates/code-review/src/prepass/secrets.rs",
      "crates/code-review/src/prepass/mod.rs",
      "crates/code-review/src/tests/prepass_test.rs",
      "tooling/conventions/guardrails/rust/rules.json"
    ],
    "input": "Minimal Shaped with runtime-built ghp_ token and a high-entropy non-matching token; low-entropy long token boundary",
    "check": "cargo test -p toolu-code-review prepass_secrets"
  },
  {
    "id": "astgrep",
    "title": "Materialize HEAD blobs and scan_inline with review-todo-macro; Missing/Failed notices via run_with",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["contract"],
    "paths": [
      "crates/code-review/src/prepass/astgrep.rs",
      "crates/code-review/src/prepass/mod.rs",
      "crates/code-review/src/prepass/rules.yaml",
      "crates/code-review/src/tests/prepass_test.rs",
      "crates/code-review/tests/prepass_contract.rs"
    ],
    "input": "Real Git TempDir with todo!() on a changed Rust line; run_with injecting Missing/Failed alongside a secret line",
    "check": "cargo test -p toolu-code-review prepass_astgrep"
  },
  {
    "id": "cap-mechanical",
    "title": "Prefer-secrets cap with overflow notice; mechanical_checks_lines helper",
    "ac_refs": ["AC-4", "AC-6"],
    "depends_on": ["secrets", "astgrep"],
    "paths": [
      "crates/code-review/src/prepass/cap.rs",
      "crates/code-review/src/prepass/mechanical.rs",
      "crates/code-review/src/prepass/mod.rs",
      "crates/code-review/src/budget.rs",
      "crates/code-review/src/tests/prepass_test.rs"
    ],
    "input": "Synthetic finding lists over PREPASS_FINDINGS_MAX; empty PrepassResult for Mechanical checks",
    "check": "cargo test -p toolu-code-review prepass_cap prepass_mechanical"
  },
  {
    "id": "docs-gate",
    "title": "Synchronize documentation and pass the complete gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["cap-mechanical"],
    "paths": [
      "crates/code-review/src/prepass.rs",
      "crates/code-review/src/prepass/mod.rs",
      "crates/code-review/src/prepass/secrets.rs",
      "crates/code-review/src/prepass/astgrep.rs",
      "crates/code-review/src/prepass/cap.rs",
      "crates/code-review/src/prepass/mechanical.rs",
      "crates/code-review/src/prepass/rules.yaml",
      "crates/code-review/src/contract.rs",
      "crates/code-review/src/budget.rs",
      "crates/code-review/src/lib.rs",
      "crates/code-review/Cargo.toml",
      "crates/code-review/src/tests/prepass_test.rs",
      "crates/code-review/tests/prepass_contract.rs",
      "crates/cli/src/commands.schema.json",
      "docs/toolu/brainstorms/2026-10-27-code-review-deterministic-pre-pass.md",
      "docs/toolu/specs/2026-10-27-code-review-deterministic-pre-pass-design.md",
      "docs/toolu/plans/2026-10-27-code-review-deterministic-pre-pass.md"
    ],
    "input": "The branch diff and the real Git / shaped fixtures from the preceding steps",
    "check": "cargo xtask gate"
  }
]
```

## Critical files

`crates/code-review/src/prepass/` (mod, secrets, astgrep, cap, mechanical, rules.yaml), `lib.rs` (`pub mod prepass`), `contract.rs`, `budget.rs`, `Cargo.toml`, `crates/cli/src/commands.schema.json`, tests, and this brainstorm/spec/plan trio.

## Plan review (2026-10-27)

No blockers. AC-1..AC-7 each appear in `ac_refs` (Jev coverage 0.75, deps 0.94). Order: contract → secrets∥astgrep → cap-mechanical → docs-gate. Real inputs declared; schema additive field in `contract` step; black-box `prepass_contract.rs` keeps `unused-pub` green for the public API. Internal stage only — no user-facing docs step beyond the doc trio in `docs-gate`.

## Verification

Focused tests prove AC-1–AC-7 with real shaped/Git inputs (no mock crates). `cargo xtask gate` verifies repository guardrails. Before implementation run `toolu ledger preflight` on this plan. After implementation, scoped commit, `toolu ledger run --verify`, local review, push, PR.
