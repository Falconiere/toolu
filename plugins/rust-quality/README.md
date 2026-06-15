# rust-quality

Rust `PostToolUse` quality checks registered into the toolu hook engine.

## Install

```
/plugin install rust-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every Rust file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks (assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart`):

- File / function / `impl` line limits (config-driven).
- No `.unwrap()` / `.expect()` — use `?` or `match`.
- No `unsafe` blocks.
- No `#[allow]` / `#[expect]` lint suppression.
- Tests in a flat `tests/`, never inline `#[cfg(test)]`.
- Doc-comment checks on public items.

The fragments register into the core toolu dispatcher and run only while this plugin is installed — uninstall it and the Rust rules vanish, fail-closed.
