# AGENTS.md

## Purpose and source of truth

toolu is a Rust plugin marketplace for Claude Code, Codex, and Muse Code. Its hooks, skills, and agent commands run the installed native `toolu` binary. This file is the agent instruction source of truth for Codex, Cursor, and Claude Code; edit it directly rather than maintaining a host-specific copy. Keep it short and current. Follow the linked source files for details that change with the implementation.

## Work in this repository

1. Find the owning crate or plugin, then read its nearby tests, fixtures, and relevant documentation before editing. Match existing patterns. The Rust workspace is in `crates/`; plugin assets are in `plugins/<name>/`.
2. Keep first-party hooks, skills, and agent commands native. Do not add TypeScript, Bun, JavaScript bundles, or handwritten shell wrappers. Generated Muse resolvers, the root `install.sh`, and the dependency-free npm wrapper at `tools/toolu-cli/npm/cli.js` are deliberate exceptions. See [runtime](docs/runtime.md) and [testing](docs/testing.md).
3. Test behavior with the real binary, Git, and host CLIs. Use the shared JSON cases in `fixtures/` when changing captured behavior. Do not use mocking crates or ignored tests. Put Rust unit tests in a sibling `tests/<module>_test.rs`, wired from the module; put black-box tests in the crate's `tests/`.
4. Run focused checks while editing and `cargo xtask gate` before pushing. Fix every gate failure; do not add suppressions or exemptions. See [Rust quality rules](docs/rust-quality-bar.md) and the root `Cargo.toml` lints. The gate requires cargo-deny, cargo-machete, cargo-llvm-cov, ast-grep, Node, and `npm ci` for jscpd.
5. Use Conventional Commits. Release versions and tags come only from release-please; do not bump or tag them manually. See [releases](docs/releases/native.md).

## Architecture and ownership

| Area | Owner | Start here |
| --- | --- | --- |
| CLI tree, hook fast path, generated command reference | `crates/cli` | `crates/cli/src/registry.rs`, `docs/cli/` |
| Plugin installation and npm wrapper | `crates/plugins`, `tools/toolu-cli/npm` | [installer](docs/cli/installer.md) |
| Host payloads, decisions, native launchers | `crates/core/protocol` | `crates/core/protocol/src/hook.rs`, `launcher.rs` |
| Host environment, config, Git detection, processes | `crates/core/runtime` | `crates/core/runtime/src/lib.rs` |
| Shell analysis, HTTP, persistent state | `crates/core/{shell,http,state}` | [shell analysis](docs/shell-analysis.md), [detection](docs/detect.md) |
| Gate dispatch, registry, quality rules, ledger | `crates/core/engine` | [registry](docs/registry.md), `crates/core/engine/src/lib.rs` |
| GitHub and Jev clients | `crates/core/{github,jev}` | Each crate's `src/lib.rs` |
| Plugin namespaces and hub commands | `crates/<plugin>`, `crates/toolu` | `crates/cli/src/registry.rs` |
| Epic engine and PR babysitter | `crates/epic-orchestrator`, `crates/pr-babysit` | [epic docs](docs/epic-orchestrator/), [babysit docs](docs/pr-babysit/) |
| Gate tasks, generated hooks, CI checks | `crates/xtask` | `crates/xtask/src/main.rs` |

Core dependency layers are `protocol` ← `runtime` ← {`shell`, `http`, `state`} ← {`engine`, `github`, `jev`}. Plugin crates depend on core; the hub connects rule crates; the CLI wires namespaces together. Check `tooling/conventions/guardrails/rust/layers.json` before adding dependencies. Keep plugin directories self-contained, without symlinks out.

## Change recipes

- **Rust crate:** Add it to root `Cargo.toml` members and `tooling/conventions/guardrails/rust/folders.json`; for a core crate, add its layer to `layers.json`. Use workspace version and lints, and a crate-level `//!` doc. Follow the test layout and [quality rules](docs/rust-quality-bar.md).
- **CLI namespace:** Export `PLUGIN`, `command()`, and `run(&ArgMatches, &Ctx) -> Outcome`; register it in `crates/cli/src/registry.rs` (rule crates go through `crates/toolu`). Run `cargo xtask docs-cli` and update the CLI contract snapshot when behavior changes. `cargo xtask check-cli-compat` gates removal of documented commands, aliases, flags, values, and exit codes; a breaking removal requires a `HOOK_PROTOCOL` increase in a `!` pull request.
- **Plugin or hook:** For a new plugin, put its Rust crate under `crates/` and assets under `plugins/<name>/`; use `tooling/templates/plugin-README.md`. Generate Claude entries with `cargo xtask print-hook <plugin> <Event> <name>`; generate Muse entries and resolvers with `cargo xtask print-muse-hook <plugin> <Event> <name>`. Do not hand-write launchers. Run `cargo xtask check-hooks`. Claude manifests carry the binary's `hookProtocol`; Muse manifests do not. For live host verification, read `.toolu/skills/verify-dual-host-plugins/SKILL.md`.
- **Rule:** Compile a first-party rule into the binary and enable it through its SessionStart manifest. Use a `QualityRule` in the matching quality crate for post-edit rules. Third-party registry rules use executable `<spec>__<name>.sh` modules. See [registry](docs/registry.md).
- **Agent-facing Markdown:** Write commands against the real `toolu` tree and run `cargo xtask check-markdown-cli`. The check covers this file, docs, skills, commands, and agents. Use [Markdown CLI guidance](docs/markdown-cli.md) for external commands and examples.
- **Gate policy:** A change to a limit, ban, lint level, or layer rule belongs in its own `chore(gates): …` PR. Registration data (`layers.json`, `folders.json`, `inventory.json`, `coverage-floor.json`) can change with the code it registers. `cargo xtask check-gate-change` enforces this split. Do not add per-path overrides.

## Verification and CI

Run `cargo test -p <package>` or `cargo test -p toolu-cli --test <file>` for a focused Rust check. `cargo xtask gate` is the complete local gate; `cargo xtask check-markdown-cli` checks agent-facing commands. `cargo xtask host-smoke --host claude|codex|muse|both` exercises native hooks in real hosts and is run manually. Read `.toolu/skills/verify-root-host-gates/SKILL.md` when verifying gates on root Linux hosts and `.toolu/skills/verify-epic-watcher/SKILL.md` when verifying the resident epic engine.

CI path groups are defined in `.github/ci-paths.json`; `cargo xtask ci-changes` selects jobs and `cargo xtask ci-aggregate tests.yml` reports their aggregate result. `.github/**`, `package-lock.json`, unmatched paths, empty diffs, and diff errors run every group. Required checks are `gate` and `review`. See `.github/workflows/tests.yml` and `.github/workflows/toolu-review.yml` for current jobs. The generated CLI contract is in `docs/cli/commands.json`.
