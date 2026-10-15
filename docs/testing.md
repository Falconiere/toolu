# Testing

Every test is a Rust test, run by `cargo test --workspace` (and by the gate's `tests` and `coverage` steps). There is no TypeScript or Bun test suite since #440. The quality gate is `cargo xtask gate`; the rules a test must follow are in [AGENTS.md](../AGENTS.md#rust-conventions) and [rust-quality-bar.md](rust-quality-bar.md).

CI runs that gate in the `rust` job. A docs-only change runs `cargo xtask gate --only context-budget --only ci-paths` and `cargo xtask check-markdown-cli`. The plugin also runs on macOS: `rust-macos` runs clippy, tests and the launcher, and `hook-bench (macos-latest)` reports hook resources.

## Real, not mocked

A test spawns the real thing: the built `toolu` (`env!("CARGO_BIN_EXE_toolu")` in `crates/cli/tests`), `git`, `npm`, `node`, `ast-grep` or a host CLI. It runs against real temporary repositories, homes and config roots. There are no mocking crates (cargo-deny bans them) and no `#[ignore]` (the guardrails reject it). A tool a test needs is installed in CI; a test that cannot find it fails rather than skips.

## Where tests live

- **Unit tests** sit beside their module in `src/<dir>/tests/<module>_test.rs`, wired by `#[cfg(test)] #[path = "tests/<module>_test.rs"] mod tests;`. They keep private access, and every module with a function has one.
- **Black-box tests** sit in each crate's root `tests/`. `crates/cli/tests` drives the built binary through its real CLI and hook entries; shared helpers are under `tests/helpers/` and are included with `#[path = "helpers/<file>.rs"] mod <name>;`. A helper returns `Result` and never unwraps; only a `#[test]` function may unwrap.
- **Fixtures** for a crate's own tests sit in `tests/fixtures/`; the language-neutral contracts several crates share sit in the repository's `fixtures/` ([fixtures/README.md](../fixtures/README.md)).

## Shared goldens

The JSON goldens in `fixtures/` were captured from the bash and TypeScript implementations the Rust crates replaced. Each is replayed against the native code, so the captured behaviour stays the contract:

| Golden | Replayed by |
|---|---|
| `dispatch/cases.json` | `crates/core/engine/tests/dispatch_fixture.rs` |
| `gates/pre-tool-modules-{a,b,c}-golden.json` and their corpus rows | `crates/cli/tests/golden_gates.rs` |
| `gates/lifecycle-golden.json` | `crates/cli/tests/golden_lifecycle.rs` |
| `quality/{ts,python,rust}-golden.json` | `crates/cli/tests/golden_quality.rs` |
| `statusline/cases.json` (render, setup, startup) | `crates/cli/tests/golden_statusline.rs` |
| `ledger/cases.json` | `crates/cli/tests/ledger_cases.rs` |
| `shell/analysis.json` | `crates/core/shell/tests/analysis_fixture.rs` |
| `host/encode.json`, `host/root.json`, `config/expected.json` | `crates/core/protocol/tests/encode_fixture.rs`, `crates/core/runtime/tests/{host_roots,config}_fixture.rs` |
| `state/{cases,gate-bytes,edit-records}.json` | `crates/core/state/tests/` |

Each replay asserts the number of cases it ran, names a mismatching case with a diff, and lists any case it deliberately does not replay in a constant with a one-line reason (for example the statusline `jev`, `report` and `report-error` kinds, which describe the retired TypeScript report). A deviation from a capture is named and asserted, never silent.

## Concurrency

Tests run in parallel. Each test owns its temporary directories (`tempfile`), never changes the process's working directory or environment, and passes `cwd` and `env` to the processes it spawns, clearing host-session variables (`CLAUDE_*`, `CODEX_*`, `TOOLU_*`, `CURSOR_*`, `PLUGIN_ROOT`) so a developer's session never leaks in. A test that writes an executable and spawns it holds the lock the launcher helpers use, so `exec` never meets `ETXTBSY` on Linux.

## Benchmarks and live checks

- `cargo xtask bench-hooks [--assert]` measures every hook entry ([resource-budgets.md](resource-budgets.md)); the `hook-bench` CI job asserts on Linux.
- `cargo xtask bench-statusline --assert` is the statusline render budget, run by the Linux `rust` leg.
- `cargo xtask host-smoke --host claude|codex|both` is the live macOS check of the native hooks in real Claude Code and Codex sessions, with Bun hidden from every child `PATH`: a denied `.env` write, a recorded quality failure, a denied commit and a denied push. It spends model credits, so it runs by hand, never in CI; see the `verify-dual-host-plugins` project skill.
- `cargo xtask launcher-e2e --bin <dir>/toolu` runs the real launcher against a binary installed in a fixed install directory; CI runs it with `sudo install`.

## Adding coverage

Add a unit test beside the module you change and a black-box test in the crate's `tests/` for a new hook surface, CLI verb or engine transition. A new `cargo xtask` task also names a passing and a failing test in `tooling/conventions/guardrails/rust/inventory.json`. For a new hook or gate, update the [coverage inventory](gate-coverage-matrix.md).
