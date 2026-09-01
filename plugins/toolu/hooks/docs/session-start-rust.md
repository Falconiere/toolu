## Rust notes
- Unit tests: module-sibling `tests/` via bodyless `#[cfg(test)] mod tests;`; crate-root `tests/` = integration.
- No `#[allow(...)]`/`#[expect(...)]` outside a `tests/` file's `#![allow]` header.
- Use `cargo nextest run`; never plain `cargo test`.
