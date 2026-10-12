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

## OpenCode

Add `rust-quality` to the project selection and restart OpenCode:

```json
{ "version": 1, "enabled": ["toolu", "rust-quality"] }
```

The project must be a git repository with `Cargo.toml` at its root, and `cargo` must be on `PATH`. The checks stay static: `cargo` is a prerequisite, never run. Completed `write`, `edit`, and `apply_patch` calls check changed `.rs` files. The installed `ast-grep` CLI runs the error-handling and no-mocks scans with its Rust parser; without it, those rules are skipped. A multi-file patch checks every changed Rust destination and clears prior entries for deleted files and moved sources. Other extensions and disabled plugins do not run these checks. Linked worktrees are checked, each against its own gate file.

Violations are appended to the completed tool result and recorded in the project's `.opencode/tmp/quality-gate-status.json`; they do not undo the edit. A failing entry blocks later commit and push attempts until a clean edit or deletion clears it. The pinned-host proof is `bun run smoke:opencode-rust-quality` in the toolu checkout.
