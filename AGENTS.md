# AGENTS.md

## Overview

**toolu** — plugin marketplace that enforces code-quality rules through hooks, skills, and a runtime registry. Runs on Claude Code, Codex, and OpenCode.

## Agent instructions

This file is the source of truth. Codex, Cursor, and Claude Code read it directly. Put instructions here. It is a docs-sync surface, so a stale copy fails the gate.

## Tech stack

- **bun test** — TypeScript suites in colocated `__tests__/*.test.ts` spawn real bundles, scripts and repos through `@toolu/conformance/harness/*`: files in parallel, tests concurrent, each test owns its sandbox. See `docs/testing.md`.
- **Bun** — the runtime for every host and plugin (1.4.x prerequisite; see `docs/runtime.md`). `bun.lock`. `bun run test` runs the TypeScript gate, including bundle drift, context budget, deterministic benchmarks, and the shell-analysis latency budget.
- **`toolu` binary** (#442) — `crates/cli`, the one Rust binary: a clap tree with a namespace per plugin crate, global `--json`/`--quiet`/`--host`/`--config-dir`, the exit codes of `toolu_protocol::exit`, and a `toolu hook` fast path that never builds the tree. `docs/cli/` is its generated reference (`cargo xtask docs-cli`), and `toolu commands --json` exports the tree.
- **Plugin installer** — `tools/toolu-cli`, a Node bundle published to npm as `@toolu/plugins` from its `npm/` folder; the workspace itself is private, so npx never mistakes it for the published package. Installs plugins across hosts by shelling out to each host's own plugin CLI, or for OpenCode by editing its documented config files. See `docs/cli/installer.md`; `toolu plugins` replaces it in #438.
- **Rust workspace** (epic #402) — root `Cargo.toml`, toolchain pinned in `rust-toolchain.toml` (1.99.0 with rustfmt and clippy). Core crates live in `crates/core/<layer>` (package `toolu-<layer>`), tooling in `crates/xtask`. Every crate passes the quality bar of #455 (see **Rust conventions** below); the gate is `cargo xtask gate`.

## Plugin layout

A plugin is a Rust crate plus Markdown. `crates/<name>` is a library that contributes one namespace, `toolu <namespace>`; the toolu hub, `crates/toolu`, contributes several and re-exports the rule crates. `plugins/<name>/` holds the skills, commands, agents, `hooks.json` and `plugin.json` that run `toolu …`. Until its port lands, a crate's namespace has one placeholder verb, `planned`, and its hooks are the Bun bundles below. Plugin packages are `toolu-<dir>`, except `toolu-review` (its directory already has the prefix) and the hub, `toolu-hub`. So `toolu-jev` is the jev plugin's, and the core Jev client of #460 takes another package name.

The plugin directory is self-contained under `plugins/<name>/`. No symlinks out.

```
plugins/<name>/
  .claude-plugin/plugin.json   # name, version, description, dependencies
  README.md                    # tooling/templates/plugin-README.md
  hooks/hooks.json             # Claude Code event routing
  hooks/src/<entry>.ts         # Bun hook or registry module source
  hooks/src/register.ts        # SessionStart: publish registry modules (hooks/dist/*.js)
  hooks/src/rules/             # a quality plugin's rules, run by its registry module
  hooks/src/__tests__/         # colocated Bun tests
  hooks/dist/*.js             # committed executable bundles
  skills/<skill>/SKILL.md
  commands/<name>.md
  agents/<name>.md
  scripts/
  settings/                    # core only
```

Root `package.json`, Bun workspace packages (`packages/toolu-core`, `tools/toolu-opencode`, `tools/toolu-conformance`, `tools/toolu-cli`), the Cargo workspace (`[workspace.package] version`), and every `plugin.json` share one `vX.Y.Z`, matching the git tag. A plugin is re-extracted only when its `plugin.json` version changes, so a release re-extracts all of them.

## Releases

release-please (`.github/workflows/release-please.yml`). No manual bumps or tags.

Any Conventional Commit on `main` counts, any path. `feat` / `fix` / `feat!` bump minor / patch / major. `chore` / `docs` / `ci` / `refactor` bump nothing. Merge the Release PR to publish: it bumps root `package.json`, Bun workspace packages under `packages/` and `tools/`, `Cargo.toml` and the workspace crates in `Cargo.lock` (TOML extra-files), and every `plugin.json`, updates `CHANGELOG.md`, tags `vX.Y.Z` with no component prefix, and opens a draft GitHub Release. OpenCode install: `docs/opencode.md`.

**npm and native releases.** `release-please.yml` creates a draft Release and a real tag, then calls `npm-publish.yml` and `release-native.yml` in the same run. npm publishes `@toolu/core`, `@toolu/opencode`, then `@toolu/plugins` with provenance via OIDC and the explicitly passed `NPM_TOKEN`; it waits until each resolves on the registry. `@toolu/conformance` stays private. The native workflow builds four binaries and publishes the draft only after checksum, SBOM, signature, version, and Linux distribution smoke checks pass. A native failure leaves the release draft even if npm shipped; see `docs/releases/native.md`. release-please also raises `@toolu/opencode`'s `^X.Y.Z` `@toolu/core` floor. `bun run test:pack` gates npm tarball file lists and closure.

## CI

| Workflow | When | What |
|----------|------|------|
| `tests.yml` | push/PR to `main`, or a manual run. No workflow-level path filter | Jobs run by path group (below); `gate` and compatibility `typescript` aggregate them |
| `release-please.yml` | push to `main` | Release PR; on merge, draft Release and tag, npm publish and native build/finalize |
| `release-native.yml` / `release-finalize.yml` | called by release-please or native dry-run dispatch | Four native archives, checksums, SPDX SBOM, provenance, OS smoke checks, then publish the draft |
| `advisory-audit.yml` | weekly schedule or manual | `cargo deny check advisories` against the latest stable tag |
| `toolu-review.yml` | PR opened/synchronize. No workflow-level path filter | `changes`, then `review` (required) with Rust prompt and Jev, followed by `merge-gate`; a same-repo generated Release PR is exempt |
| `merge-gate.yml` | PR label or review activity | Refreshes `merge-gate` after a reply or `merge-approved` label without a push |

Path groups live in `.github/ci-paths.json` (#458). A `changes` job runs `tooling/src/ci-changes.ts`, which turns on the groups the diff touches:

- `pull_request`: base...head;
- `push`: before..after;
- `workflow_dispatch`: every group.

Each gated job carries a job-level `if`, and a job skipped that way reports Success. A required check therefore never stays Pending.

| Job | Group | Runs |
|-----|-------|------|
| `changes` | — | `tooling/src/ci-changes.ts`: one `true`/`false` output per group, plus `changed` |
| `ts` (`bun run test`) | `ts` | `bun run test` (`test:ts`): format, lint, typecheck, guardrails, gate reach, legacy exemptions, unit, conformance, bundle/launcher drift, Markdown–CLI drift (`cargo xtask check-markdown-cli`, so the job installs the pinned toolchain), CI path check, context and latency budgets, deterministic benchmarks |
| `opencode (ubuntu-latest)`, `opencode (macos-latest)` | `opencode` | `bun run test:opencode`, the real OpenCode acceptance |
| `docs` | `docs` | `bun run test:docs`: the checks and tests from `test:ts` that read `docs/**` or root Markdown, the Markdown–CLI drift gate included (a verb rename regenerates `docs/cli/**`, so it reaches this group) |
| `rust`, `rust-macos` | `rust` | Linux runs suppression and layer checks, then the full `cargo xtask gate --base <PR base or pushed range> --title <PR title>` with cargo-deny, cargo-machete, cargo-llvm-cov, ast-grep and Bun. macOS 14 runs clippy, Rust tests and launcher e2e. Linux also checks the startup budget |
| `rust-musl (x86_64-unknown-linux-musl)`, `rust-musl (aarch64-unknown-linux-musl)` | `rust` | A release build per musl target (aarch64 on `ubuntu-24.04-arm`); `file` must report the binary static |
| `rust-conformance` | `ports` | `bun run test:rust-conformance`: the ported hook entries in `fixtures/rust-ported.json` against `test:unit` and `test:conformance` with `TOOLU_IMPL` set (#409). An empty list is a no-op and installs no toolchain |
| `hook-bench (ubuntu-latest)`, `hook-bench (macos-latest)` | `ports` | Runs `hook-resources.native.test.ts`, then `bun run bench:hooks`, the max RSS, CPU and wall of every hook entry per spawn (#410). Linux adds `--assert`, which gates the entries in `fixtures/rust-ported.json` against `benchmarks/hook-budgets.json`; macOS only reports. Each leg uploads its JSON result. See `docs/resource-budgets.md` |
| `review` | `changed` | The code review. Runs for any change outside release-only files, docs included, and for forks even with a release-only diff; a generated same-repo Release PR is exempt |
| `gate`, `typescript` | aggregate, `if: always()` | `tooling/src/ci-aggregate.ts`. Both evaluate all gated jobs identically. `typescript` remains for existing branch protection while `gate` is added |

These paths turn every group on:

- `.github/**`, which includes the data file;
- `bun.lock`;
- a path no group matches;
- an empty diff, or a diff error.

A release-please bump turns every group off. That means release-only paths whose diff is only `version`, `.` or `@toolu/core` semver lines (JSON, or TOML `version = "X.Y.Z"` in `Cargo.toml`/`Cargo.lock`), plus `CHANGELOG.md`. The `rust` group is `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `lefthook.yml`, `crates/**`, `.cargo/**`, the two `toolu.config.json` copies, `fixtures/guardrails/**`, `tooling/conventions/guardrails/rust/**`, `tooling/src/check-rust-quality.ts`, `plugins/rust-quality/**`, every plugin's `hooks/hooks.json` and `plugin.json` (for `cargo xtask check-hooks`), `docs/cli/**` and `benchmarks/startup-budgets.json` (#442); #408 adds the rest of `fixtures/**`. The `ports` group is the port list and its runner, the hook bench (`tooling/src/benchmarks/hook-resources.ts`, `lib/hook-*.ts`, its tests, `benchmarks/cases/hooks/**`, `benchmarks/hook-budgets.json`), `tools/toolu-conformance/**`, `plugins/**`, `packages/**` and the Rust paths. `bun run check:ci-paths` fails on any of these:

- a workflow-level `paths`/`paths-ignore` where a required check is reported;
- an aggregate whose `needs` differ from its gated jobs;
- a gated job without a group;
- a glob that matches no tracked file.

Benchmarks are hermetic. Context budget caps the Session Protocol, per-language docs, and skill descriptions.

## Rust conventions

Every crate under `crates/` passes `cargo xtask gate` (#455), the required `rust` CI job on Linux; the `rust-macos` leg runs clippy, tests and OS checks. The full gate also runs by `lefthook.yml` (fast checks on commit, the gate on push) and, after each edit, by the rust-quality plugin at this repository's limits (`lang.rust` in `.claude/toolu.config.json`). Code lines exclude blank lines and comments. Reasons: [`docs/rust-quality-bar.md`](docs/rust-quality-bar.md).

| # | Rule | Limit | Owner |
|---|---|---|---|
| 1 | File length, `src` and tests | 300 code lines | guardrails |
| 2 | Function length | 50 code lines | clippy `too_many_lines` |
| 3 | `impl` block length | 200 code lines | guardrails |
| 4 | Complexity | cognitive 15, nesting 4, 5 parameters, 2 `bool` parameters, 3 `bool` fields | clippy |
| 5 | Formatting | width 100, two spaces per level, no tabs | `cargo fmt --check` |
| 6 | Test layout | no inline `#[cfg(test)]` body in `src`; unit tests in `tests/<module>_test.rs` beside the module, wired by `#[cfg(test)] #[path = "tests/<module>_test.rs"] mod tests;`; `tests/` flat except `fixtures/`, `helpers/`, `common/` | guardrails |
| 7 | Co-located tests | a module file with a function has a wired `tests/<module>_test.rs` with a test | guardrails |
| 8 | Behaviour inventory | each discovered item (today every `cargo xtask` task) names a passing and a failing test in `inventory.json` | guardrails |
| 9 | Coverage | 85% of lines per crate, 90% for `toolu-protocol`, `toolu-shell`, `toolu-state`, `toolu-engine`; floors only rise | llvm-cov, `check-coverage` |
| 10 | Real tests | no mocking crates, no `#[ignore]` | cargo-deny, guardrails |
| 11 | Folders | allowlists for the root, `crates/`, each crate and each plugin; snake_case files; no mod files, build scripts or source includes; ≤3 levels under `src`; `main.rs` only in `cli`/`xtask`; `#[path]` only for test wiring | guardrails, clippy |
| 12 | Crate hygiene | `[lints] workspace = true`, `version.workspace = true`, crate `//!` doc | guardrails |
| 13 | Layers | `protocol` ← `runtime` ← {`shell`, `http`, `state`} ← {`engine`, `github`, `jev`}; plugins link only core | `check-layers` |
| 14 | Capabilities | HTTP/TLS crates only in `toolu-http`, shell parsers only in `toolu-shell`; env, process and stdio only in their owners; `process::exit` only in `main` | `check-layers`, guardrails, clippy |
| 15 | Dependencies | no async runtime, OpenSSL, `anyhow`/`eyre`, duplicate versions, unknown licence or source, unused dependency, or `dyn Error` in a library's `pub fn` | cargo-deny, cargo-machete, guardrails |
| 16 | Duplication | 0 clones of 10 lines / 60 tokens in `crates/**/src` | jscpd |
| 17 | No unsafe | `unsafe_code = "forbid"` | rustc |
| 18 | No panics in `src` | `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, indexing, string slicing denied outside tests | clippy |
| 19 | No leftovers | no `dbg!`; a to-do marker names its issue (`#<n>`) | clippy, guardrails |
| 20 | No suppression | no `allow`/`expect` attribute (outer, inner or in `cfg_attr`), no jscpd ignore comment | guardrails |
| 21 | No dead code | warnings denied, `unreachable_pub`, every `pub` item used by another crate or a test | rustc, `check-unused-pub` |
| 22 | Documentation | `missing_docs`, rustdoc warnings denied | rustc, `cargo doc` |
| 23 | Secrets | no committed secret | guardrails |

- **Data.** Gate data (a limit, a ban, a lint level, a layer rule): `[workspace.lints]`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `lang.rust`, `tooling/conventions/guardrails/rust/{rules,jscpd}.json`. It changes only in its own `chore(gates): …` PR. Registration data may grow next to the code it registers: `layers.json`, `folders.json`, `inventory.json`, `coverage-floor.json` (rows at or above the default). `cargo xtask check-gate-change` enforces the split. No file has an ignore, exempt or per-path override field; the loaders reject one.
- **Fixtures.** `fixtures/guardrails/rust/<rule>/{clean,violating*}`: each case is an overlay on `base/` with an `expect.txt`; Rust sources end in `.rs.fixture`. `cargo test -p xtask` runs every case through `cargo xtask gate --root <temp> --only <step>` with this repository's real gate data.
- **Commands.** `cargo xtask gate` (all steps, or `--only <step>`), `cargo xtask guardrails`. `cargo xtask docs-cli` rewrites `docs/cli/` from the binary, and the gate's `docs-cli` step checks it. `cargo xtask check-cli-compat` (the gate's `cli-compat` step) fails when a documented command, alias, flag, value or exit code disappears, unless `HOOK_PROTOCOL` rises in a `!` pull request; the placeholder `planned` verbs are exempt. `cargo xtask check-startup --bin <toolu>` gates the `toolu --version` budget. `cargo xtask check-markdown-cli` fails when a skill, command, agent, `AGENTS.md` or doc names a `toolu` command, verb or flag that `docs/cli/commands.json` lacks, runs an external command off its allow-list, or runs a removed surface of a ported namespace; it runs in `bun run test` and `test:docs` until #439 ([`docs/markdown-cli.md`](docs/markdown-cli.md)). `cargo xtask check-workflows` checks workflow triggers, permissions, action pins, targets, and release chaining. The gate needs cargo-deny, cargo-machete, cargo-llvm-cov, ast-grep, Bun and `bun install`; a missing tool fails closed.

## Key files

| File | Purpose |
|------|---------|
| `plugins/toolu/hooks/src/pre-tools.ts` | Pre-tool dispatcher, bundled to `hooks/dist/pre-tools.js`: all nine built-in modules are native (`NATIVE_MODULES` in `pre-tools/builtins.ts`), then `pre-tools.d`. Golden captures replay the former Bash gates. `hooks/src/mcp-tools.ts` is the `mcp__` hook; `hooks/src/agent-tier.ts` handles delegated agents. |
| `packages/toolu-core/src/dispatch/dispatch.ts` | `@toolu/core/dispatch`: `dispatchPreTool` and `dispatchPostTool` (deny over ask over advisory before a tool, block over advisory after it, per-path patch walk). Registry ESM modules run in process. |
| `plugins/toolu/hooks/src/post-tools.ts` | Post-tool dispatcher, bundled to `hooks/dist/post-tools.js`: native gate-status and push-waiver, then `post-tools.d` (language-quality checks on edited files). |
| `packages/toolu-core/src/gates/gates.ts` | `@toolu/core/gates`: native pre-tool gates (bash-commands, commit-gate, quality-gate, protected-files, mcp-blocker, code-edit-rules, push-review, plan-ledger, docs-sync) and post-tool gates (gate-status, push-waiver), plus parsed-command helpers. `@toolu/core/gates/mcp-hook` and `@toolu/core/gates/agent-tier` are standalone entries. |
| `plugins/pr-babysit/hooks/src/babysit-tick.ts` | Babysit tick, bundled to `hooks/dist/babysit-tick.js`. Writes go through the bundled reply, resolve and record entries |
| `plugins/pr-babysit/hooks/src/babysit-route-fix.ts` | Bun fixer routing bundle; scores Fix items with Jev and groups them by host, model and effort. `babysit-dispatch-fix.ts` starts and waits for herdr fixer agents, and for detached `opencode run` fixers (`babysit/fixer-process.ts`). |
| `plugins/*/hooks/src/register.ts` | SessionStart registry sync |
| `plugins/*/hooks/hooks.json` | Claude Code hook routing |
| `tools/toolu-cli/src/cli.ts` | CLI entry (`npx @toolu/plugins install`, or `toolu install` once installed): parses argv, resolves the host, dispatches a verb |
| `tools/toolu-cli/src/plugins/install.ts` | Dependency-ordered install; core failure stops dependents, others continue |
| `tools/toolu-cli/src/host/` | Per-host adapters normalizing `plugin list --json` into one shape |
| `tools/toolu-cli/src/opencode/` | OpenCode management without host commands: JSONC edits of the `plugin` array (global or project, merged the way the pinned host merges them) and the `toolu/plugins.json` selection. Live proof: `bun run smoke:opencode-entry cli.install cli.lifecycle` |
| `tooling/src/pack-inventory.ts` | Published-tarball file-list gate over `npm pack --dry-run --json`; `@toolu/opencode` is packed from a temp copy through its own prepack (`tooling/src/npm-pack.ts`) and closure-checked (`tooling/src/pack-closure.ts`) |
| `tooling/src/guardrails/run.ts` | Structural gate (`bun run guardrails`): TypeScript port of the vendored conventions runner; data in `tooling/conventions/guardrails/` |
| `tooling/src/check-gate-reach.ts` | Reach gate (`bun run check:gate-reach`): every tracked TypeScript file is reached by typecheck, format, oxlint, jscpd and knip, or `tooling/gate-reach.json` declares the gap; also checks that each `ownedByLinter` id is a rule the package lint config runs. Code in `tooling/src/gate-reach/` |
| `tooling/src/check-legacy-exemptions.ts` | Stale-exemption gate (`bun run check:legacy-exemptions`): re-runs oxlint, jscpd and knip with exact-path exemptions lifted; an exemption whose finding is gone fails. See `docs/conventions-adoption.md` |
| `tooling/src/check-retired-plugin-references.ts` | Retired-plugin reference gate (`bun run check:retired-plugins`): rejects new mentions of the four #406 standalone plugins outside reviewed history, the unknown-plugin test, and the built-in Jira integration. |
| `tooling/src/opencode-host-probe.ts` | Live OpenCode host probes (`bun run probe:opencode-host`): the pinned `opencode-ai` CLI in isolated profiles against a scripted loopback provider. Evidence lives in `tools/toolu-opencode/contract/probe-results.json`; contract in `docs/opencode-host-contract.md` |
| `tooling/src/opencode-acceptance.ts` | Required OpenCode acceptance (`bun run test:opencode`, CI on Linux and macOS). It runs the pinned host through the contract probes, every live scenario and `*.live.test.ts` file, concurrency, a startup/per-tool budget (`contract/acceptance-budgets.json`) and four staged regression controls. It fails on a missing tool, a skip, an undetected control or a plugin without a dedicated actual-host check, and writes a JSON report that keeps host, fixture and live-service evidence apart. Code in `tooling/src/opencode-acceptance/`; see `docs/conformance-report.md` |
| `tooling/src/opencode-host-contract.ts` | Hermetic OpenCode host-contract check (`bun run check:opencode-host`, in `test:portable-core`): the pin, the probe evidence, the 12-plugin capability matrix, the plugin manifests, the pinned SDK declarations and the contract doc must agree |
| `tooling/src/check-opencode-docs.ts` | OpenCode docs check (`bun run check:opencode-docs`, in `test:portable-core`): regenerates `docs/opencode.md`'s per-plugin support section from the capability matrix and the acceptance registry, and rejects V2-only claims on the install path. The `docs.*` acceptance checks run the install and migration guides' marked bash blocks verbatim (`tooling/src/opencode-host/scenarios-docs*.ts`) |
| `tools/toolu-conformance/src/harness/entry-command.ts` | The `TOOLU_IMPL` seam (#409): tests reach a committed hook bundle only through it (`bundlePath`, `entryArgv`, `publishedArgv`, `launchedArgv`), so a selected entry runs the Rust `toolu` binary instead. `bundle-references.test.ts` enforces it; see `docs/testing.md` |
| `tooling/src/benchmarks/run.ts` | Benchmark harness (`bun run benchmarks`); inputs, fixtures and committed results in `benchmarks/` |
| `tooling/src/benchmarks/hook-resources.ts` | Hook resource bench (`bun run bench:hooks [--assert]`, #410). It measures every `hooks.json` entry with the fixed payloads in `benchmarks/cases/hooks/` through `cargo xtask measure`, which spawns one command and reads `getrusage(RUSAGE_CHILDREN)`. It gates ported entries against `benchmarks/hook-budgets.json`. Budgets and method: `docs/resource-budgets.md` |
| `packages/toolu-core/src/launcher/launcher.ts` | `@toolu/core/launcher`: the generated `hooks.json` command that runs a bundle with Bun and fails closed without it |
| `packages/toolu-core/src/host/host.ts` | `@toolu/core/host`: host detection (Claude, Codex, Cursor, OpenCode, Hermes), roots, Codex plugin snapshot, event-name map, per-host output encoders and `ask` degradation |
| `packages/toolu-core/src/config/config.ts` | `@toolu/core/config`: `toolu.config.json` loader (fail-closed envelope), thresholds, gate modes, permissions write, `settings/*` loaders; bash parity over `fixtures/config` |
| `packages/toolu-core/src/registry/registry.ts` | `@toolu/core/registry`: bundled ESM hook modules in `<config>/toolu/<dir>.d/`; module contract, in-process runner (gating, isolation, stop after deny), SessionStart register and Codex prune. See `docs/registry.md` |
| `packages/toolu-core/src/state/state.ts` | `@toolu/core/state`: multi-slot gate file (locked atomic writes, strict v1 Zod), state sweeper, `diffSha`, closed-schema telemetry, edit-record normalization; byte parity with the bash libs |
| `packages/toolu-core/src/ledger/ledger.ts` | `@toolu/core/ledger`: plan ledger CLI, parse, preflight, verdict gates and push waivers |
| `packages/toolu-core/src/startup/startup.ts` | `@toolu/core/startup`: what leaf-plugin SessionStart hooks share: stable-path publishing, Bun-on-PATH advisory, bounded context output, Codex dependency warnings, and the startup report records (`TOOLU_STARTUP_REPORT`) the OpenCode bootstrap verifies |
| `packages/toolu-core/src/shell/shell.ts` | `@toolu/core/shell`: parses a Bash/Shell command once with unbash (pinned) into simple commands (wrappers unwrapped, `bash -c`/`eval` followed), git subcommand/`-C` chain/push destination and exit observability; write targets in `@toolu/core/shell/writes`; parity fixtures in `fixtures/shell`, budgets via `bun run bench:shell` (`docs/shell-analysis.md`) |
| `packages/toolu-core/src/detect/detect.ts` | `@toolu/core/detect`: port of `detect.sh`: project markers and linters, tool availability (PATH scan, cached), chunked code-line counts, branch slug/base, and push/commit, push root and branch from a `ShellAnalysis`; loads neither unbash nor zod; bash parity on real repos (`docs/detect.md`) |
| `packages/toolu-core/src/quality/quality-edit.ts` | Post-edit quality helpers: identify the edited file and delete/move status, check whether it is a regular file, and detect linked worktrees |
| `tooling/src/check-hooks-json.ts` | `hooks.json` launcher gate (`bun run check:hooks-json`); `--print <plugin> <Event> <entry>` emits the entry to paste |
| `tooling/src/build-plugins.ts` | Builds `plugins/*/hooks/src` entries into committed `hooks/dist` bundles; `--check` is the drift gate |
| `crates/core/protocol/src/launcher.rs` | `toolu_protocol::launcher`: the generated native `hooks.json` command (#412). It finds `toolu` (`TOOLU_BIN`, then the install directories, then `PATH`), requires `--hook-protocol` to print an integer, maps an enforcing event's crash to exit 2, falls back to the Bun bundle until #440, and otherwise fails closed. `HOOK_PROTOCOL` is in the crate root; see `docs/install.md` |
| `crates/core/protocol/src/hook.rs` | `toolu_protocol::hook::run_hook` (#413), the main every native hook calls: it reads stdin, runs the hook in `catch_unwind`, encodes its `Decision` for the host (`encode.rs`, a port of `host-encode.ts` with Codex `ask` degradation) or writes a dispatcher's raw output, and returns exit 0 or 2, never 101. A panic, an internal error or a lost write blocks with exit 2 on `PreToolUse` and `PermissionRequest`, exits 2 on `PostToolUse` and `SessionEnd`, and leaves a `systemMessage` with exit 0 on `SessionStart`, `UserPromptSubmit` and `PreCompact`. The same crate holds the open host payloads (`payload.rs`, unknown keys kept), their mapping (`normalize.rs`) to the strict `NormalizedEvent` (`normalized.rs`), `Decision` and its precedence (`decision.rs`), and each host's event names (`native.rs`). `fixtures/host/encode.json` is the byte-for-byte encoder contract that the Rust and TypeScript encoders share |
| `crates/core/runtime/src/lib.rs` | `toolu-runtime` (#414): what every binary needs to run on a host, without a shell parser or TLS. `env::Env` is the explicit environment snapshot; `host` ports `@toolu/core/host` (detection, `Roots`, the Codex plugin snapshot); `config` ports `@toolu/core/config` (the fail-closed envelope with TypeScript's messages, the namespaced `epic` key, gate modes, thresholds, the permissions write, `settings/*`); `git` (#415, `toolu_runtime::git`) finds the toplevel, git dir and common dir by walking up for `.git` as git does, so `Roots::project_root` spawns nothing (it asks git only for the `GIT_DIR` variables, `sudo`, another owner, includes, `config.worktree` and quoted values); `process` runs commands in their own process group with a deadline (`nix` `killpg`), the only `std::process::Command` owner; `startup` ports the stable-path publish (symlinks only, never a user's file), bounded context and `TOOLU_STARTUP_REPORT`; `registry` holds the `<spec>__<name>.json` manifest and the `Rule` trait. `fixtures/config/expected.json` and `fixtures/host/root.json` are its TypeScript parity goldens |
| `crates/core/state/src/lib.rs` | `toolu-state` (#415): every on-disk state file, byte for byte as `@toolu/core/state` writes it, so TypeScript and Rust hooks share them. `gate_file` is the multi-slot gate file (strict v1, JavaScript key order, unrecognized documents replaced with a drop log) under `lock`, the TypeScript `<file>.lock` protocol (pid and UUID, dead or 2 s stale holders broken, unlocked after 5 s); `telemetry` is a closed event enum; `edit_records` and `apply_patch` normalize edit payloads; `sweeper` reclaims spent state; `diff_sha` hashes the branch diff; `git` reads the branch, linked worktree, common dir and origin HEAD from `.git`; `detect` ports the shell-free half of `@toolu/core/detect`. Goldens `fixtures/state/{gate-bytes,edit-records}.json`; `tests/interleave.rs` races TypeScript and Rust writers on one file |
| `crates/cli/src/main.rs` | The `toolu` binary (`toolu-cli`). `fast.rs` answers `--hook-protocol` and `[<plugin>] hook <name> --event <Event> --plugin-root <dir>` without building the clap tree. Everything else goes through the tree (`tree.rs`, the namespaces and owners in `registry.rs`, `dispatch.rs`), which holds the output contract: data on stdout, diagnostics on stderr, one JSON document under `--json`. The hook path applies the #411 skew rule from the plugin's `plugin.json` (`toolu-runtime`'s `manifest` and `skew`), then runs the hook; only toolu's `session-start` diagnostic is native so far. `toolu commands --json` and `--schema` (`export.rs`, `commands.schema.json`) feed `docs/cli/` |
| `docs/cli/` | The generated CLI reference (#442): one page per command from its real `--help`, plus `commands.json` and `commands.schema.json`. `installer.md` is the hand-written guide of the Node installer |
| `Cargo.toml` | Rust workspace: explicit `members` (a `crates/*` glob would also match `crates/core`), lockstep version, shared dependencies, base lints, release profile (`lto = "fat"`, `codegen-units = 1`, `strip`, `panic = "unwind"`) |
| `crates/xtask/src/main.rs` | `cargo xtask <task>` (`TASKS`): `gate`, `guardrails`, `check-layers`, `check-reach`, `check-unused-pub`, `check-gate-change`, `check-coverage`, `measure` (the hook bench's measurer), `print-hook`, `check-hooks`, `check-workflows`, `launcher-e2e`, `docs-cli`, `check-cli-compat`, `check-startup`, `check-markdown-cli`; see **Rust conventions**. `print-hook <plugin> <Event> <name> [--timeout N]` prints a native `hooks.json` entry; `check-hooks` (the gate's `hooks` step) fails when a native entry differs from it or lacks a `timeout`, or a `plugin.json` lacks the binary's `hookProtocol`; `launcher-e2e --bin <dir>/toolu` runs the real launcher against an installed binary. `check-layers` reads `cargo metadata --no-deps`, the layer table `tooling/conventions/guardrails/rust/layers.json` and the capability crates of `rules.json`: a core crate may use only lower core layers, a plugin crate only core, only `crates/toolu` may use a rule crate, only `crates/cli` (and `crates/xtask`) builds a binary, nothing uses `cli` or `xtask`, and every `crates/<name>` or `crates/core/<name>` with a `Cargo.toml` is a member. Normal and build dependencies are judged; dev-dependencies are not. Exit 0 clean, 1 violations, 2 setup error |
| `docs/config.md` | Config schema |
| `plugins/toolu/scripts/context-budget.ts` | Injected-context word ceilings (`bun run test:context-budget`) |

## Contributing

1. Match an existing skill, agent, command, or hook.
2. Colocate `__tests__/*.test.ts` for Bun tests. No mocks.
3. Verify in a real session, then commit with `feat(scope):` or `fix(scope):`.
4. `bun run test` before pushing.

- Skill: `plugins/<name>/skills/<skill>/SKILL.md`
- Quality rule: `plugins/<quality>/hooks/src/rules/` plus a colocated `bun test` case.
- Hook module: a `defineRegistryModule` entry in `plugins/<plugin>/hooks/src/`, listed in its `hooks/src/register.ts`
- TypeScript hook: `plugins/<name>/hooks/src/<entry>.ts` (a top-level file is an entry; helpers go in subdirectories). `bun run build:plugins` writes the self-contained `hooks/dist/<entry>.js`; commit both. Wire it in `hooks.json` with the generated launcher (`bun run tooling/src/check-hooks-json.ts --print <plugin> <Event> <entry>`), never a hand-written `bun` call; `bun run check:hooks-json` gates it. `bun run check:plugin-bundles` (in `test:ts`) fails when a bundle drifts from its source, is missing, or is orphaned. Build with the pinned Bun (CI: 1.4.2).
- Skill CLI: an entry starting `#!/usr/bin/env bun` builds to an executable bundle (the drift check covers the exec bit) that a SessionStart hook symlinks to a stable path, e.g. the Jev wrapper. HTTP helpers use `@toolu/core/rest` where needed by the epic tracker; tests run against real loopback HTTPS fixtures through `HTTPS_PROXY`.
- Native hook entry: paste `cargo xtask print-hook <plugin> <Event> <name>` into `hooks.json`, never a hand-written `toolu` call; `cargo xtask check-hooks` gates it, and every `plugin.json` carries `"hookProtocol"` equal to `toolu_protocol::HOOK_PROTOCOL`.
- Rust crate: add its directory to `members` in the root `Cargo.toml` (a core crate also needs its layer in `tooling/conventions/guardrails/rust/layers.json`), list it in `folders.json`, set `version.workspace = true` and `[lints] workspace = true`, give the crate root a `//!` doc, and follow **Rust conventions**. `cargo xtask gate` must pass with no exemption.
- Plugin: `plugins/<name>/.claude-plugin/plugin.json`, a README from `tooling/templates/plugin-README.md`, and its crate `crates/<name>`.
- CLI namespace: the plugin crate exports `PLUGIN`, `command()` and `run(&ArgMatches, &Ctx) -> Outcome` and is listed in `crates/cli/src/registry.rs` (a rule crate through `crates/toolu`). Then run `cargo xtask docs-cli`, and accept the `commands --json` snapshot with `INSTA_UPDATE=always cargo test -p toolu-cli --test contract`. Removing or renaming a documented command, alias, flag or value bumps `HOOK_PROTOCOL` in a `!` pull request.
- Markdown that runs a command: write plain `toolu …` against the real tree; `cargo xtask check-markdown-cli` judges it. A new external tool goes in `external` of `tooling/conventions/markdown-cli.json`, and an intentional example of wrong usage goes in a `text` fence or, with a reason, an `allow` entry ([`docs/markdown-cli.md`](docs/markdown-cli.md)).
- Agent-facing `toolu` commands use plain `toolu …`. SessionStart and `toolu doctor` check reachability with `/bin/sh -c 'command -v toolu'` and the agent command's `PATH`; they validate the resolved binary with `--hook-protocol`. Each relevant plugin stays silent when that probe finds native `toolu`, or supplies one absolute path or install advisory for the session. The npm plugin installer still has a Node `toolu` wrapper until #438; the native check rejects it. Hooks keep their generated launcher.
- Subset: `bun test plugins/<plugin>/hooks/src/__tests__/`
- New TypeScript tree: add it to `tsconfig.json`, `format:check`, a lint config, `.jscpd.json` and `knip.json`, or declare the gap in `tooling/gate-reach.json`; `bun run check:gate-reach` fails otherwise. Never add a legacy exemption for new code.

Version is `package.json` and every `plugin.json`. License: MIT.
