# Refactor a Rust Repo into toolu Compliance
Invoke the `rust-refactor` skill on the target path (`$ARGUMENTS`, default cwd). Always report → confirm → apply; never auto-apply. Follow the skill's phases in order with NO extra exploration:

1. **Preflight** — run `${CLAUDE_PLUGIN_ROOT}/scripts/rust-refactor-preflight.sh --path <path>`. If it exits non-zero (dirty tree / non-Cargo / non-git), STOP and report — do not touch the tree.
2. **Audit** — `${CLAUDE_PLUGIN_ROOT}/scripts/rust-scan.sh --path <path> --json` + `cargo fmt --all --check` + `cargo clippy --all-targets` (capture, no fix). Present the report grouped by autofix class. STOP.
3. **Confirm** — user approves all or a subset; enforce config-before-clippy ordering (reject decline-config + accept-clippy-fix).
4. **Apply** — mechanical classes via `${CLAUDE_PLUGIN_ROOT}/scripts/rust-refactor-apply.sh --path <path>`; restructure classes agent-driven, each `cargo check`-gated and rolled back on failure; skip `#[path]`/`include!`/macro modules as `manual`.
5. **Final gate** — `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo nextest run` (abort with `cargo install cargo-nextest` hint if absent).
6. **Report** — diff summary + `manual` residuals.

Do not mark complete unless the final gate is green.
