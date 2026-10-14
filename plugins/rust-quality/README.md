# rust-quality

Rust `PostToolUse` quality checks compiled into `toolu` and enabled by a registry manifest.

## Install

**Prerequisite:** the native [`toolu` binary](../../docs/install.md).

```
/plugin install rust-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Rust file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are the `toolu-rust-quality` crate (`crates/rust-quality`), compiled into `toolu`. The `SessionStart` register hook publishes its manifest into toolu's registry, plus a small `.sh` bridge that lets the Bun post-tools dispatcher run the same rule until #425. It also removes the Bun module of earlier releases once, with a notice:

- File / function / `impl` line limits (config-driven).
- No `.unwrap()` / `.expect()` — use `?` or `match`.
- No `unsafe` blocks.
- No `#[allow]` / `#[expect]` lint suppression — except a `tests/` file's file-level `#![allow(...)]` header, for the lints test code violates on purpose.
- Unit tests in a module-sibling `tests/` (wired by a bodyless `#[cfg(test)] mod tests;`, `#[path]`-attributed or not); crate-root `tests/` for integration. No inline test bodies in `src/`.
- Doc-comment checks on public items.

The rule runs in the core toolu dispatcher only while this plugin is installed — uninstall it and the Rust rules vanish, fail-closed. `toolu rust-quality --help` lists the checks, and `toolu rust-quality check [<path>...]` runs them over a repository's tracked and untracked `.rs` files, the way this repository's gate does.
