# AGENTS.md

## Overview

**toolu** — plugin marketplace that enforces code-quality rules through hooks, skills, and a runtime registry. Runs on Claude Code, Codex, and OpenCode.

## Agent instructions

This file is the source of truth. Codex, Cursor, and Claude Code read it directly. Put instructions here. It is a docs-sync surface, so a stale copy fails the gate.

## Tech stack

- **bun test** — TypeScript suites in colocated `__tests__/*.test.ts` spawn real bundles, scripts and repos through `@toolu/conformance/harness/*`: files in parallel, tests concurrent, each test owns its sandbox. See `docs/testing.md`.
- **Bun** — the runtime for every host and plugin (1.4.x prerequisite; see `docs/runtime.md`). `bun.lock`. `bun run test` runs the TypeScript gate, including bundle drift, context budget, deterministic benchmarks, and the shell-analysis latency budget.
- **`toolu` CLI** — `tools/toolu-cli`, a Node bundle published to npm as `@toolu/plugins` from its `npm/` folder; the workspace itself is private, so npx never mistakes it for the published package. Installs plugins across hosts by shelling out to each host's own plugin CLI, or for OpenCode by editing its documented config files. See `docs/cli.md`.
- **Rust workspace** (epic #402) — root `Cargo.toml`, toolchain pinned in `rust-toolchain.toml` (1.99.0 with rustfmt and clippy). Core crates live in `crates/core/<layer>` (package `toolu-<layer>`), tooling in `crates/xtask`. `rustfmt.toml` uses two spaces per indentation level, no tabs, width 100. `[workspace.lints]` denies warnings and `unwrap`/`expect`/`panic!` outside tests (`clippy.toml` allows them in tests); the full rule set is #455. The gate is `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` and `cargo xtask check-layers`.

## Plugin layout

Self-contained under `plugins/<name>/`. No symlinks out.

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

Any Conventional Commit on `main` counts, any path. `feat` / `fix` / `feat!` bump minor / patch / major. `chore` / `docs` / `ci` / `refactor` bump nothing. Merge the Release PR to publish: it bumps root `package.json`, Bun workspace packages under `packages/` and `tools/`, `Cargo.toml` and the workspace crates in `Cargo.lock` (TOML extra-files), and every `plugin.json`, updates `CHANGELOG.md`, tags `vX.Y.Z` with no component prefix, and opens the GitHub Release. OpenCode install: `docs/opencode.md`.

**npm.** `release-please.yml` calls `npm-publish.yml` after the Release, which publishes `@toolu/core`, `@toolu/opencode`, then `@toolu/plugins` with provenance via OIDC and `secrets.NPM_TOKEN`, and does not go green until each one resolves on the registry (a first publish can take minutes to stop answering 404). `@toolu/conformance` stays `private`; it is an internal harness. release-please also raises `@toolu/opencode`'s `^X.Y.Z` `@toolu/core` floor, and the publish step refuses a tag that does not match it. `bun run test:pack` gates each tarball's file list as `npm pack` reports it, and checks that `@toolu/opencode` is closed over its own tarball (`tooling/src/pack-closure.ts`).

## CI

| Workflow | When | What |
|----------|------|------|
| `tests.yml` | push/PR to `main`, or a manual run. No workflow-level path filter | Jobs run by path group (below); `typescript` is the required aggregate |
| `release-please.yml` | push to `main` | Release PR; on merge, tag and GitHub Release |
| `toolu-review.yml` | PR opened/synchronize. No workflow-level path filter | `changes`, then `review` (required): `falconiere/toolu-ghactions/code-review@v8` (Jev on: `JEV_ENABLED` + `JEV_MODEL_ID: typesafe/jev-1.13`) |

Path groups live in `.github/ci-paths.json` (#458). A `changes` job runs `tooling/src/ci-changes.ts`, which turns on the groups the diff touches:

- `pull_request`: base...head;
- `push`: before..after;
- `workflow_dispatch`: every group.

Each gated job carries a job-level `if`, and a job skipped that way reports Success. A required check therefore never stays Pending.

| Job | Group | Runs |
|-----|-------|------|
| `changes` | — | `tooling/src/ci-changes.ts`: one `true`/`false` output per group, plus `changed` |
| `gate` (`bun run test`) | `ts` | `bun run test` (`test:ts`): format, lint, typecheck, guardrails, gate reach, legacy exemptions, unit, conformance, bundle/launcher drift, CI path check, context and latency budgets, deterministic benchmarks |
| `opencode (ubuntu-latest)`, `opencode (macos-latest)` | `opencode` | `bun run test:opencode`, the real OpenCode acceptance |
| `docs` | `docs` | `bun run test:docs`: the checks and tests from `test:ts` that read `docs/**` or root Markdown |
| `rust (ubuntu-latest)`, `rust (macos-latest)` | `rust` | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build` and `cargo test --workspace --locked`, `cargo xtask check-layers` |
| `rust-musl (x86_64-unknown-linux-musl)`, `rust-musl (aarch64-unknown-linux-musl)` | `rust` | A release build per musl target (aarch64 on `ubuntu-24.04-arm`); `file` must report the binary static |
| `rust-conformance` | `ports` | `bun run test:rust-conformance`: the ported hook entries in `fixtures/rust-ported.json` against `test:unit` and `test:conformance` with `TOOLU_IMPL` set (#409). An empty list is a no-op and installs no toolchain |
| `review` | `changed` | The code review. Runs for any change outside the release-only files, docs included, and runs anyway if `changes` failed or left `changed` empty |
| `typescript` | aggregate, `if: always()` | `tooling/src/ci-aggregate.ts`. Fails when `changes` failed or left a group output that is not `true`/`false`, a needed job failed or was cancelled, or a job was skipped while its group was on |

These paths turn every group on:

- `.github/**`, which includes the data file;
- `bun.lock`;
- a path no group matches;
- an empty diff, or a diff error.

A release-please bump turns every group off. That means release-only paths whose diff is only `version`, `.` or `@toolu/core` semver lines (JSON, or TOML `version = "X.Y.Z"` in `Cargo.toml`/`Cargo.lock`), plus `CHANGELOG.md`. The `rust` group is `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `crates/**` and `.cargo/**`; #408 adds `fixtures/**` with the directory. The `ports` group is the port list and its runner, `tools/toolu-conformance/**`, `plugins/**`, `packages/**` and the Rust paths. `bun run check:ci-paths` fails on any of these:

- a workflow-level `paths`/`paths-ignore` where a required check is reported;
- an aggregate whose `needs` differ from its gated jobs;
- a gated job without a group;
- a glob that matches no tracked file.

Benchmarks are hermetic. Context budget caps the Session Protocol, per-language docs, and skill descriptions.

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
| `tooling/src/opencode-host-probe.ts` | Live OpenCode host probes (`bun run probe:opencode-host`): the pinned `opencode-ai` CLI in isolated profiles against a scripted loopback provider. Evidence lives in `tools/toolu-opencode/contract/probe-results.json`; contract in `docs/opencode-host-contract.md` |
| `tooling/src/opencode-acceptance.ts` | Required OpenCode acceptance (`bun run test:opencode`, CI on Linux and macOS). It runs the pinned host through the contract probes, every live scenario and `*.live.test.ts` file, concurrency, a startup/per-tool budget (`contract/acceptance-budgets.json`) and four staged regression controls. It fails on a missing tool, a skip, an undetected control or a plugin without a dedicated actual-host check, and writes a JSON report that keeps host, fixture and live-service evidence apart. Code in `tooling/src/opencode-acceptance/`; see `docs/conformance-report.md` |
| `tooling/src/opencode-host-contract.ts` | Hermetic OpenCode host-contract check (`bun run check:opencode-host`, in `test:portable-core`): the pin, the probe evidence, the 16-plugin capability matrix, the plugin manifests, the pinned SDK declarations and the contract doc must agree |
| `tooling/src/check-opencode-docs.ts` | OpenCode docs check (`bun run check:opencode-docs`, in `test:portable-core`): regenerates `docs/opencode.md`'s per-plugin support section from the capability matrix and the acceptance registry, and rejects V2-only claims on the install path. The `docs.*` acceptance checks run the install and migration guides' marked bash blocks verbatim (`tooling/src/opencode-host/scenarios-docs*.ts`) |
| `tools/toolu-conformance/src/harness/entry-command.ts` | The `TOOLU_IMPL` seam (#409): tests reach a committed hook bundle only through it (`bundlePath`, `entryArgv`, `publishedArgv`, `launchedArgv`), so a selected entry runs the Rust `toolu` binary instead. `bundle-references.test.ts` enforces it; see `docs/testing.md` |
| `tooling/src/benchmarks/run.ts` | Benchmark harness (`bun run benchmarks`); inputs, fixtures and committed results in `benchmarks/` |
| `packages/toolu-core/src/launcher/launcher.ts` | `@toolu/core/launcher`: the generated `hooks.json` command that runs a bundle with Bun and fails closed without it |
| `packages/toolu-core/src/host/host.ts` | `@toolu/core/host`: host detection (Claude, Codex, Cursor, OpenCode, Hermes), roots, Codex plugin snapshot, event-name map, per-host output encoders and `ask` degradation |
| `packages/toolu-core/src/config/config.ts` | `@toolu/core/config`: `toolu.config.json` loader (fail-closed envelope), thresholds, gate modes, permissions write, `settings/*` loaders; bash parity over `tooling/fixtures/config` |
| `packages/toolu-core/src/registry/registry.ts` | `@toolu/core/registry`: bundled ESM hook modules in `<config>/toolu/<dir>.d/`; module contract, in-process runner (gating, isolation, stop after deny), SessionStart register and Codex prune. See `docs/registry.md` |
| `packages/toolu-core/src/state/state.ts` | `@toolu/core/state`: multi-slot gate file (locked atomic writes, strict v1 Zod), state sweeper, `diffSha`, closed-schema telemetry, edit-record normalization; byte parity with the bash libs |
| `packages/toolu-core/src/ledger/ledger.ts` | `@toolu/core/ledger`: plan ledger CLI, parse, preflight, verdict gates and push waivers |
| `packages/toolu-core/src/startup/startup.ts` | `@toolu/core/startup`: what leaf-plugin SessionStart hooks share: stable-path publishing, Bun-on-PATH advisory, bounded context output, Codex dependency warnings, and the startup report records (`TOOLU_STARTUP_REPORT`) the OpenCode bootstrap verifies |
| `packages/toolu-core/src/shell/shell.ts` | `@toolu/core/shell`: parses a Bash/Shell command once with unbash (pinned) into simple commands (wrappers unwrapped, `bash -c`/`eval` followed), git subcommand/`-C` chain/push destination and exit observability; write targets in `@toolu/core/shell/writes`; parity fixtures in `tooling/fixtures/shell`, budgets via `bun run bench:shell` (`docs/shell-analysis.md`) |
| `packages/toolu-core/src/detect/detect.ts` | `@toolu/core/detect`: port of `detect.sh`: project markers and linters, tool availability (PATH scan, cached), chunked code-line counts, branch slug/base, and push/commit, push root and branch from a `ShellAnalysis`; loads neither unbash nor zod; bash parity on real repos (`docs/detect.md`) |
| `packages/toolu-core/src/quality/quality-edit.ts` | Post-edit quality helpers: identify the edited file and delete/move status, check whether it is a regular file, and detect linked worktrees |
| `tooling/src/check-hooks-json.ts` | `hooks.json` launcher gate (`bun run check:hooks-json`); `--print <plugin> <Event> <entry>` emits the entry to paste |
| `tooling/src/build-plugins.ts` | Builds `plugins/*/hooks/src` entries into committed `hooks/dist` bundles; `--check` is the drift gate |
| `Cargo.toml` | Rust workspace: explicit `members` (a `crates/*` glob would also match `crates/core`), lockstep version, shared dependencies, base lints, release profile (`lto = "fat"`, `codegen-units = 1`, `strip`, `panic = "unwind"`) |
| `crates/xtask/src/main.rs` | `cargo xtask <task>`. `check-layers` reads `cargo metadata --no-deps` and the layer table `crates/xtask/layers.json`: a core crate may use only lower core layers, a plugin crate only core, only `crates/toolu` may use a rule crate, only `crates/cli` (and `crates/xtask`) builds a binary, nothing uses `cli` or `xtask`, and every `crates/<name>` or `crates/core/<name>` with a `Cargo.toml` is a member. Normal and build dependencies are judged; dev-dependencies are not. Exit 0 clean, 1 violations, 2 setup error |
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
- Skill CLI: an entry starting `#!/usr/bin/env bun` builds to an executable bundle (the drift check covers the exec bit) that a SessionStart hook symlinks to a stable path, e.g. `hooks/src/search.ts` for exa-search and context7. HTTP goes through `@toolu/core/rest`; tests run the bundle against `@toolu/conformance/https-fixture`, a loopback HTTPS server reached through `HTTPS_PROXY`.
- Rust crate: add its directory to `members` in the root `Cargo.toml` (a core crate also needs its layer in `crates/xtask/layers.json`), set `version.workspace = true` and `[lints] workspace = true`, and put tests under the crate's `tests/`. `cargo xtask check-layers` fails on an unlisted crate directory.
- Plugin: `plugins/<name>/.claude-plugin/plugin.json` and a README from `tooling/templates/plugin-README.md`
- Subset: `bun test plugins/<plugin>/hooks/src/__tests__/`
- New TypeScript tree: add it to `tsconfig.json`, `format:check`, a lint config, `.jscpd.json` and `knip.json`, or declare the gap in `tooling/gate-reach.json`; `bun run check:gate-reach` fails otherwise. Never add a legacy exemption for new code.

Version is `package.json` and every `plugin.json`. License: MIT.
