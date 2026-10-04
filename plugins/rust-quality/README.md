# rust-quality

Rust `PostToolUse` quality checks registered into the toolu hook engine.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install rust-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Rust file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks run as one bundled registry module, `hooks/dist/post-tool-use.js`, that `SessionStart` publishes to the registry:

- File / function / `impl` line limits (config-driven).
- No `.unwrap()` / `.expect()` — use `?` or `match`.
- No `unsafe` blocks.
- No `#[allow]` / `#[expect]` lint suppression — except a `tests/` file's file-level `#![allow(...)]` header, for the lints test code violates on purpose.
- Unit tests in a module-sibling `tests/` (wired by a bodyless `#[cfg(test)] mod tests;`, `#[path]`-attributed or not); crate-root `tests/` for integration. No inline test bodies in `src/`.
- Doc-comment checks on public items.

The module runs in the core toolu dispatcher only while this plugin is installed — uninstall it and the Rust rules vanish, fail-closed.

## OpenCode

Add `rust-quality` to the project selection and restart OpenCode:

```json
{ "version": 1, "enabled": ["toolu", "rust-quality"] }
```

The project must be a git repository with `Cargo.toml` at its root, and `cargo` must be on `PATH`. The checks stay static: `cargo` is a prerequisite, never run. Completed `write`, `edit`, and `apply_patch` calls check changed `.rs` files. The installed `ast-grep` CLI runs the error-handling and no-mocks scans with its Rust parser; without it, those rules are skipped. A multi-file patch checks every changed Rust destination and clears prior entries for deleted files and moved sources. Other extensions and disabled plugins do not run these checks. Linked worktrees are checked, each against its own gate file.

Violations are appended to the completed tool result and recorded in the project's `.opencode/tmp/quality-gate-status.json`; they do not undo the edit. A failing entry blocks later commit and push attempts until a clean edit or deletion clears it. The pinned-host proof is `bun run smoke:opencode-rust-quality` in the toolu checkout.
