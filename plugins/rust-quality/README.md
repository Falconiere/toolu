# rust-quality

Rust `PostToolUse` quality checks registered into the toolu hook engine.

## Install

```
/plugin install rust-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Rust file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks (assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart`):

- File (250) / function (80) / `impl` (200) line limits (config-driven).
- No `.unwrap()` / `.expect()` — use `?` or `match`.
- No `unsafe` blocks.
- No `#[allow]` / `#[expect]` lint suppression.
- `mod.rs` carries only `mod` / `pub use` / docs — no logic.
- No generic file names (`utils`/`helpers`/`common`/`misc`; `shared/`, `common/` dirs exempt).
- A module-level `//!` doc header (a missing `# Public API` / `# Usage` section is advisory).
- Best-effort layering: per-file `pub(super)` / deep-`use` checks block; cross-file reach is advisory.
- Tests in a flat `tests/`, never inline `#[cfg(test)]`.

The gate fires on the **nearest** enclosing `Cargo.toml`, so a nested workspace crate is covered, not just the repo root. Thresholds and the new rules are tunable via `lang.rust.{maxFileLines,maxFnLines,maxImplLines,forbiddenFileNames,modRsNoLogic,requireModuleDoc,enforceLayering}` in `toolu.config.json`.

The fragments register into the core toolu dispatcher and run only while this plugin is installed — uninstall it and the Rust rules vanish, fail-closed.

## `/rust-refactor`

A repo-wide command that brings an existing Cargo workspace into compliance: it audits the crate (`rust-scan.sh`), scaffolds clippy / rustfmt / cargo-deny / lefthook / CI config (`rust-scaffold.sh`, merge-not-clobber, with `[workspace.lints]` propagated to every member), and restructures code to the rules above. It runs **preflight → audit → confirm → apply → report** and never auto-applies; mechanical fixes (`cargo fmt`, `cargo clippy --fix`) are commit-then-`cargo check`-then-rollback, and the structural restructure is agent-driven with the same compile-checked rollback per step. Repo-wide error-handling / `unsafe` / suppression enforcement is handled by the scaffolded `cargo clippy` denies; the per-edit gate keeps a fast ast-grep subset.

> **Note:** the structural-scan step in the scaffolded `lefthook.yml` / CI workflow calls `rust-scan.sh`, which ships with this toolu plugin and is **not** on a target repo's `PATH` by default. The templates therefore resolve it and **skip the structural scan gracefully** (with an echoed note) when it isn't installed — the native `cargo fmt` / `clippy` / `nextest` steps always run. A future enhancement may vendor `rust-scan.sh` into the target repo.
