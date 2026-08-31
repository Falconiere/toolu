## Rust notes
- Unit tests: module-sibling `tests/` via bodyless `#[cfg(test)] mod tests;`; crate-root `tests/` = integration.
- No `#[allow(...)]`/`#[expect(...)]` — fix the warning, don't suppress.
- Use `cargo nextest run`; never plain `cargo test`.
