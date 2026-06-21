---
name: rust-refactor
description: "Refactor a Rust repo into toolu compliance — enforce structure + linters/formatter, scaffold clippy + rustfmt + cargo-deny + lefthook + CI, and bring an existing crate up to the 250/80 structural rules. Use when asked to 'refactor a Rust repo', 'enforce Rust structure', 'scaffold clippy/rustfmt/lefthook/CI', 'bring this crate into compliance', or '/rust-refactor <path>'."
---

# rust-refactor

Brings a target Cargo workspace into toolu-native compliance: native config
(clippy/rustfmt/cargo-deny) + structural rules (file-size 250, `mod.rs`-no-logic,
generic-name ban, test-location, module `//!` docs, layering) + wiring (lefthook +
CI). Always **report → confirm → apply** — never auto-applies.

Target path is the argument (default: cwd). The workspace need not be at repo
root (nested `packages/backend/` layouts resolve).

The mechanical pipeline is scripted (`scripts/rust-refactor-*.sh`); the semantic
restructure (choosing split boundaries / renames) is your judgement below.

## Phase 1 — Preflight

Run `scripts/rust-refactor-preflight.sh --path <repo>`.

- It aborts NON-ZERO on a **dirty tree** — if it exits non-zero, **STOP** and
  tell the user to commit or stash first. Do not proceed, do not fix the tree.
- It also errors on a non-Cargo / non-git target — STOP and report.
- On success it has created/checked-out branch `rust-quality/refactor`. All
  subsequent work lands there.

## Phase 2 — Audit (read-only)

Capture the full picture; mutate nothing.

- `scripts/rust-scan.sh --path <repo> --json` — structural violations + config
  presence/absence.
- `cargo fmt --all --check` (capture; do not fix).
- `cargo clippy --all-targets` (capture; do not fix).

Present a **categorized report grouped by autofix class** (`fmt`, `clippy-fix`,
`config`, `restructure`/`split`/`rename`/`move`, `manual`) with counts and the
config gaps. **STOP** and wait for the user.

## Phase 3 — Confirm

The user approves **all**, or a **subset** of autofix classes.

- **Ordering is enforced.** `config` (scaffold) MUST run before `clippy-fix`:
  the deny lints define what `clippy --fix` repairs. Declining `config` while
  accepting `clippy-fix` is **rejected** — explain the dependency rather than
  silently produce a partial result.
- `restructure` may be accepted independently of the mechanical classes.
- Apply subsets strictly in dependency order: config → fmt → clippy-fix →
  restructure.

## Phase 4 — Apply

**Mechanical classes** (`config`, `fmt`, `clippy-fix`): run
`scripts/rust-refactor-apply.sh --path <repo>`. It scaffolds/merges config +
propagates `[workspace.lints]`, runs `cargo fmt --all`, runs
`cargo clippy --fix --allow-dirty --allow-staged --all-targets`, then re-audits
and prints residuals by autofix class. Each step is committed → `cargo check` →
rolled back on failure (the AC-11 mechanism, built in).

**Restructure classes** (`restructure`/`split`/`rename`/`move`) are AGENT-DRIVEN.
Do each as its own step, then run `cargo check`; if a step breaks the build,
`git reset --hard HEAD~1` it and record it as a `manual` residual (mirror the
script's `_apply_step` discipline — never leave the tree half-applied):

- **Extract inline tests** — move `#[cfg(test)]` modules out of `src/` into the
  crate's flat `tests/`.
- **Flatten nested tests** — `tests/integration/x_test.rs` → `tests/x_test.rs`
  (keep `fixtures/`, `helpers/`, `common/` support dirs).
- **Rename generic-named files** — `utils.rs`/`helpers.rs`/`common.rs`/`misc.rs`
  → a name that states the concern; rewrite the parent `mod`/`use`.
- **Split oversized files** — convert a `>250`-code-line file to a
  `dir/mod.rs` folder module and split by concern; the `mod.rs` carries only
  `//!` + `mod` + `pub use` (no logic).
- **Rewrite `mod`/`use`** and add `pub(super)` where a child item is exposed
  wider than its parent needs.
- **Add module `//!` docs** with `# Public API` / `# Usage` sections.

**SKIP** any file whose module mapping is non-standard — `#[path = "…"]`
attributes, `include!()`, or macro-generated modules — recording it as a
`manual` residual rather than risk corrupting a non-file-based module tree.

## Phase 5 — Final gate

Install the lefthook + CI wiring (the scaffold step already did), then run:

```
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo nextest run
```

If `cargo-nextest` is absent, **abort** with the install hint:
`cargo install cargo-nextest` (no `cargo test` fallback). Fix any gate failure
and re-run until fully green.

## Phase 6 — Report

- Diff summary of what landed on `rust-quality/refactor`.
- The `manual` residuals — files skipped (`#[path]`/macro modules) and steps
  rolled back — so the user knows exactly what to finish by hand.
