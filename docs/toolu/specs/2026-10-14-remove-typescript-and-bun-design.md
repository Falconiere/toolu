# Remove the TypeScript implementation and the Bun prerequisite — Design

**Date:** 2026-10-14   **Status:** Approved   **Author:** Claude (epic worker, #440)   **Topic:** Delete the remaining TypeScript tree, the Bun fallback and every Bun step, so toolu runs on Claude Code and Codex with only the native binary.

## Problem

Every hook already has a native implementation (#418–#433, #445, #459–#463), yet the repository still ships:

- the TypeScript core (`packages/toolu-core`, 224 files) and the conformance harness (`tools/toolu-conformance`, 78 files);
- toolu's Bun bundles (`plugins/toolu/hooks/{src,dist}`);
- a Bun fallback and `TOOLU_RUNTIME=bun` in every generated `hooks.json` command (`crates/core/protocol/src/launcher.rs`);
- a Bun bridge for `.js` registry modules (`crates/core/engine/src/registry/bridge*`);
- stable-path shell shims (`jev.sh`, `statusline.sh`, `write-state.sh`);
- about 160 TypeScript tooling files (`tooling/src`). Several of them still run in CI: the hook bench, the statusline bench, `ci-changes.ts` in `toolu-review.yml`, and the workflow check that `check_workflows.rs` spawns.

`cargo xtask gate` itself spawns Bun in five steps. Contributors and CI must install Bun, the plugin tree carries megabytes of bundles, and two implementations of every rule drift apart. Issue #440 is the epic's last sub-issue: delete all of this and document the native-only result.

## Non-Goals

1. A Windows binary. Windows `commandWindows` entries report that no native `toolu` exists. The release targets stay the four macOS and Linux archives (#411).
2. Changing any gate decision, state-file format, registry ordering, telemetry or host permission policy.
3. Rewriting historical `docs/toolu/{brainstorms,specs,plans}` files that cite deleted TypeScript paths. They are records.
4. Porting the TypeScript-only manual tools that no gate, workflow or skill runs: the live LLM token bench (`bench live`), the Linux container latency tier (`bench container`), `clean-install-smoke.ts`, `codex-smoke.ts`, `pr-babysit-herdr-smoke.ts`, `probe-ask.ts` and the retired guardrails, reach and lint scripts. They are deleted, and their `cargo xtask bench` modes are removed.
5. Merging or releasing. The epic orchestrator owns merge order. Precondition: **merge only after a release that contains #425 (the native switch) has been published**, so the Bun fallback ships for one release as epic #402 requires. The PR body and the `ready` report state this.
6. Removing the `@toolu/plugins` npm wrapper. It keeps publishing.

## Architecture

The work is deletion plus a handful of small Rust ports for what still runs. Every surviving behaviour has a Rust owner, and each deletion is checked by the gate's `final-removal` step.

1. **Launcher (`toolu_protocol::launcher`).** `command` keeps the native lookup (`TOOLU_BIN`, the install directories, then `PATH`, each probed with `--hook-protocol`) and the enforcing exit mapping. It drops the Bun fallback, the bundle path and the `TOOLU_RUNTIME` guard, so a missing binary goes straight to the existing fail-closed or `systemMessage` branch. `commandWindows` becomes only that missing-binary branch, written for cmd.exe. `install::fallback_advisory` is deleted. Every `plugins/*/hooks/hooks.json` entry is regenerated with `cargo xtask print-hook`. `cargo xtask check-hooks` accepts only native entries, because `bun_launcher.rs` and the Bun branch of `hooks_entries.rs` are deleted.
2. **Registry (`toolu-engine`).** The `.js` bridge (`registry/bridge.rs`, `bridge/{bun,runner}.rs`, `phase/batching.rs`) is deleted. A `.js` entry is still listed and gated, but it never runs. It is traced as `Skipped(Skip::Unsupported)`, writes no per-call stderr, and no longer shadows a same-spec `.sh` module. `toolu hook session-start` gains one advisory line per `.js` file found in `<config>/toolu/{pre,post}-tools.d`. Third-party modules use the executable protocol, unchanged from #418: `bash <file>.sh`, the raw hook text on stdin, the module environment, hook-format JSON on stdout, and exit 2 as a hard deny. `docs/registry.md` documents it.
3. **Quality plugins.** `QualityPlugin::register` (`crates/core/engine/src/quality/plugin.rs`) writes only the manifest. It deletes the `.sh` bridge it used to write, as it already does with old `.js` modules, and `bridge_script` is deleted.
4. **Stable-path shims.** jev, statusline and toolu-review stop publishing `jev.sh`, `statusline.sh` and `write-state.sh`. Their SessionStart removes the symlink toolu published there, which is a symlink into a plugin root, and leaves a regular user file alone. The jev mandate text and the skills name `toolu jev …`. The statusline settings migration (`setup::migrate`, a legacy `statusline.sh` command rewritten to `toolu statusline render`) stays, so a late upgrader is migrated before the shim disappears. `plugins/*/scripts/*.sh` are deleted. `toolu doctor` drops its Bun check.
5. **xtask without Bun.**
   - `ci_yaml` parses YAML with a pure-Rust crate and converts it to `serde_json::Value`. The crate must pass `cargo deny`.
   - `check_workflows` ports `tooling/src/{check-workflows,workflow-checks,install-channel-checks}.ts` rule for rule.
   - The `dist` gate step and `dist check` are deleted. `dist checksums` stays.
   - The `workspace` and `portable-core` steps are deleted, with `docs/portable-core.md`.
   - `pack` checks only `@toolu/plugins`.
   - `gate-coverage` rows are all `native`.
   - `final-removal` is rewritten to the new invariant (Interfaces).
   - `bench` keeps only `deterministic`.
   - New task `bench-hooks [--assert] [--out <file>]` ports `hook-resources.ts`. It measures every `hooks.json` entry with `benchmarks/cases/hooks/payloads.json` through the existing `measure`, and gates the entries in `benchmarks/hook-budgets.json`.
   - New task `bench-statusline [--assert]` ports `benchmarks/statusline.ts`.
   - New task `host-smoke` ports `native-toolu-host-smoke.ts`, removes Bun from the child `PATH`, and adds a push-review step.
   - `markdown_cli` keeps flagging `bun <script>` in Markdown.
6. **npm wrapper.** `tools/toolu-cli/npm/cli.js` is a committed, dependency-free ESM file translated from `src/{cli,resolve}.ts` with identical behaviour. It needs no build step. `package.json` sets `bin: {"toolu-plugins": "cli.js"}`, and its `prepack` copies `marketplace.json` with `cp`. The wrapper tests move to `crates/cli/tests/npm_wrapper.rs`, which runs `node cli.js` against the built `toolu`. `tools/toolu-cli/{src,package.json,tsconfig.json,…}` are deleted.
7. **Deleted trees.** `packages/`, `tools/toolu-conformance`, `plugins/toolu/hooks/{src,dist}`, `plugins/*/skills/__tests__`, `plugins/toolu/scripts/context-budget.ts` and its test, `plugins/toolu/skills/setup/scripts`, all of `tooling/src`, `tooling/{package.json,tsconfig.json,latency.Dockerfile,gate-reach.json,guardrails.config.json}`, and `tooling/fixtures` except `native-launcher/` (read by `launcher-e2e`). The TypeScript-only conventions go too. Rule: a `tooling/` file survives only when a crate, a workflow, `lefthook.yml` or a plugin reads it. The known survivors are `tooling/conventions/guardrails/rust/`, `tooling/conventions/markdown-cli{.json,/}`, `tooling/templates/` and `tooling/fixtures/native-launcher/`. Also deleted: root `bun.lock`, `tsconfig*.json`, `knip.json`, oxlint configs, `fixtures/rust-ported.json`, and the `TOOLU_IMPL` seam.
   - Rust tests that spawn Bun for parity (`helpers/{fallback,interop,resolution}.rs`, `ledger_interop`, `ledger_leases`, `setup_agents_parity`, `waiver_interop`, `pre_gates_interleave`, `engine/tests/bridge.rs`, the state TypeScript writers and interleave) keep their golden-fixture halves and drop the live-TypeScript half.
   - The `delivery-flow` Markdown contract tests move to `crates/xtask/tests/skills_contract.rs`, with the same assertions.
8. **Root toolchain.** A minimal private `package.json` keeps the release-please `node` version anchor and the `jscpd` devDependency, locked by `package-lock.json`. CI runs `npm ci`. The gate's jscpd hint says `npm ci`.
9. **CI.**
   - `tests.yml` drops `setup-bun` and the `rust-conformance` job. `hook-bench` runs `cargo xtask bench-hooks`, and the statusline budget runs `cargo xtask bench-statusline --assert`.
   - The `ports` group in `.github/ci-paths.json` becomes the bench paths. `runEverything` names `package-lock.json` instead of `bun.lock`. The aggregates' `needs` follow.
   - `toolu-review.yml` runs `cargo xtask ci-changes`.
   - `npm-publish.yml` publishes only `tools/toolu-cli/npm`, with npm, and drops Bun.
   - `release-native.yml` drops Bun.
   - `release-please-config.json` drops `packages/toolu-core`, `tools/toolu-conformance` and `tools/toolu-cli/package.json`.
10. **Docs.**
    - Rewritten or updated: `docs/runtime.md` (native only), `AGENTS.md` (tech stack, plugin layout, key files, contributing), `tooling/templates/plugin-README.md`, `docs/testing.md`, `docs/registry.md`, `docs/resource-budgets.md`, `docs/install.md`, and the plugin READMEs that mention shims.
    - `docs/cli/` is regenerated: the `hook` help text no longer names `hooks/dist`.
    - The breaking changes go in `BREAKING CHANGE:` footers of the `feat!` commit, which release-please copies into `CHANGELOG.md`.

The decisive trade-off is deleting the live TypeScript oracle. The shared JSON goldens in `fixtures/` (dispatch, shell, state, host, config, ledger, quality) stay, and each was captured from TypeScript and is replayed by Rust tests. They remain the contract, so no parity is lost except through live re-capture, and nothing will ever change the TypeScript side again.

## Interfaces / Schema

- **Launcher.** POSIX `command` (enforcing event):

  ```sh
  t=; <lookup>; if [ -n "$t" ]; then "$t" <argv>; s=$?; case $s in 0|2) exit $s;; esac; <abnormal>; exit 2; fi
  printf '%s\n' 'blocked: <missing>' >&2; exit 2
  ```

  For a context event, the first branch is `exec` and the second prints `{"systemMessage":"<missing>"}` and exits 0. `commandWindows` is `(1>&2 echo blocked: <missing>& exit /b 2)` for an enforcing event and `echo {"systemMessage":"<missing>"}` otherwise. No `bun`, `TOOLU_BUN`, `TOOLU_RUNTIME` or `hooks/dist` appears in any generated string.
- **Registry advisory.** One stderr line on SessionStart per module: `toolu-registry: <dir>/<file> is a JavaScript module, which toolu no longer runs; publish it as an executable <spec>__<name>.sh (docs/registry.md#executable-modules)`. `<dir>` is `pre-tools.d` or `post-tools.d`. The line goes into the hook's `systemMessage`, so the user sees it once per session start. `trace::Skip::Unsupported` serialises as `"unsupported"` in `toolu hook … --trace` output.
- **`cargo xtask bench-hooks [--assert] [--runs N] [--out <file>]`.** Writes `benchmarks/results/hook-resources-rust-<os>-<arch>-<date>.json` in the shape of the existing `hook-resources-rust-linux-x64-ci-2026-10-13.json`. It exits 1 when `--assert` finds an entry over its budget, and 2 on a setup error.
- **`cargo xtask bench-statusline [--assert]`.** Same budget numbers as `statusline.ts`: 30 runs, 3 warm-up runs, wall p50 ≤ 4000 µs, RSS ≤ 5 MiB. It writes the JSON the TypeScript wrote.
- **`cargo xtask host-smoke [--bin <toolu>] [--host claude|codex|both] [--keep]`.** macOS only. Children get `PATH` without any directory that contains `bun`. It checks the protected `.env` deny, an allowed `pwd` control, the post-edit quality failure state, the quality-gated commit denial (HEAD unchanged), and a push denied by push-review against a local bare remote (the remote ref unchanged). It exits 0 when all pass, 1 on a failed check, and 2 on a setup error.
- **`final-removal` invariant.** It fails when any of these holds:
  - `git ls-files` lists a `*.ts`, `*.tsx`, `*.mts` or `*.cts` outside `benchmarks/fixtures/`, or any `bun.lock`, `bun.lockb` or `bunfig.toml`;
  - a `plugins/*/hooks/dist` or `plugins/*/hooks/src` exists;
  - `packages/` or `tools/toolu-conformance` exists;
  - any tracked `*.sh` remains except `install.sh`;
  - a `hooks.json` command or a workflow names `bun`, `setup-bun` or `TOOLU_RUNTIME`;
  - root `package.json` has `workspaces`;
  - the `typescript` aggregate job is missing from `tests.yml`. It stays for branch protection.

  The coverage inventory rows must have `hostMechanism: native`.
- **npm wrapper.** `npm pack --dry-run` lists exactly `LICENSE`, `README.md`, `assets/marketplace.json`, `cli.js` and `package.json`. Behaviour is unchanged: `TOOLU_BIN` alone when set, else the install directories and `PATH`, never itself. It answers `--hook-protocol` and runs `toolu plugins <verb> --catalog <assets/marketplace.json> …`, or exits 69 with the install commands.

## Failure modes and edge cases

- **No native binary** (any platform, Bun installed or not): enforcing hooks exit 2 with `blocked: …install…`, and context hooks emit the install `systemMessage` and exit 0. Bun is never run.
- **`TOOLU_RUNTIME=bun` set by a former rollback user:** ignored, and native runs. `docs/runtime.md` says so.
- **`TOOLU_BIN` pointing at a bad file:** fails closed, as today. There is no Bun fallback any more even when `TOOLU_BIN` is unset.
- **`.js` registry module present.** It is never spawned and the walk continues. A same-spec `.sh` now runs. The advisory repeats on every session start until the file is gone, but it is not repeated per tool call.
- **Unreadable registry directory:** no advisory and no error, as `list_dir` already does.
- **Shim path holds a user's regular file or a foreign symlink:** left untouched, and no error. Removing toolu's own symlink fails, for example on a read-only directory: SessionStart reports one warning line and does not block.
- **YAML the Rust parser rejects**, such as anchors or a tab indent: `ci-paths` and `workflows` fail with the file name and parser message, never silently pass.
- **`bench-hooks` on a host without a hook's prerequisites** (for example `ast-grep` missing): the entry's measurement is reported and the budget still applies. A spawn failure is exit 2.
- **`host-smoke` with no Claude or Codex login:** exit 2 naming the missing host. A host turn timeout after 180 s counts as a failed check.
- **npm wrapper on Node < 20.12:** the `engines` field refuses it, as today.
- **Concurrent hooks:** unchanged. The Bun/Rust gate-file interleave tests go away because only Rust writes the file; the Rust-only race test (`tests/interleave.rs` Rust half) stays.

## Acceptance criteria

- **AC-1:** After the change, `git ls-files '*.ts' '*.tsx' '*.mts' '*.cts'` prints only `benchmarks/fixtures/` data. Nothing prints for `bun.lock`, `packages/`, `tools/toolu-conformance`, `plugins/*/hooks/src`, `plugins/*/hooks/dist`, `fixtures/rust-ported.json` or `plugins/*/scripts/*.sh`. `cargo xtask gate --only final-removal` passes on the branch and fails on a fixture that re-adds any one of them.
- **AC-2:** Every `plugins/*/hooks/hooks.json` entry equals `cargo xtask print-hook` output and contains no `bun`, `TOOLU_BUN`, `TOOLU_RUNTIME` or `hooks/dist`. With `TOOLU_BIN` unset, no `toolu` on the search path, and a real `bun` on `PATH`, the toolu `PreToolUse` entry exits 2 with the install message, and SessionStart prints the install `systemMessage` and exits 0. With the built binary it runs native.
- **AC-3:** Consider a config dir whose `toolu/pre-tools.d` holds `x@t__deny.js`, which would deny, and `x@t__ok.sh`, which allows, with plugin `x@t` installed. A `PreToolUse` Bash payload through `toolu hook pre-tools` is allowed and never spawns `bun`. `toolu hook session-start` prints exactly one advisory naming `pre-tools.d/x@t__deny.js` and `docs/registry.md`.
- **AC-4:** In a config dir where an earlier version published `jev/jev.sh`, `statusline/statusline.sh` and `toolu-review/write-state.sh` as symlinks into the plugin, running each plugin's SessionStart removes those symlinks. A regular user file at the same path survives byte for byte, and the jev context names `toolu jev`, not `jev.sh`.
- **AC-5:** After `toolu ts-quality hook session-start` (and the python and rust equivalents) in a config dir that holds the old `.sh` bridge, `post-tools.d` contains only that plugin's `.json` manifest.
- **AC-6:** `cargo xtask gate` passes with a `PATH` that contains no `bun`, as does every `cargo test --workspace` target.
- **AC-7:** `cargo xtask bench-hooks --assert` measures every `hooks.json` entry and passes the `benchmarks/hook-budgets.json` budgets. It exits 1 when a budget is lowered below a measured value in a temp copy. A new `benchmarks/results/hook-resources-rust-…json` is committed next to the existing `hook-resources-bun-*-2026-10-05.json` baselines.
- **AC-8:** `npm-publish.yml` and `release-please-config.json` contain no `packages/toolu-core` or `tools/toolu-conformance`, and they still publish `tools/toolu-cli/npm` (`@toolu/plugins`). `cargo xtask gate --only pack` passes with the file list above. `crates/cli/tests/npm_wrapper.rs` passes: resolution order, `--hook-protocol`, the `plugins install --help` passthrough, and exit 69 without a binary.
- **AC-9:** On macOS with `bun` absent from every child `PATH`, `cargo xtask host-smoke --host both` passes against the built binary in real Claude Code and Codex sessions. It covers a denied `.env` write, a recorded quality failure, a denied commit with HEAD unchanged, and a denied push with the remote ref unchanged. On Linux, the `rust` CI job runs with no Bun installed and passes `launcher-e2e` and the hook tests.
- **AC-10:** `cargo xtask check-markdown-cli` and `cargo xtask gate --only docs-cli` pass. `git grep -n -E '\bbun (run|install|test|build|x)\b|setup-bun|hooks/dist|TOOLU_RUNTIME|@toolu/core' -- AGENTS.md README.md 'docs/*.md' 'docs/cli/**' tooling/templates 'plugins/**/*.md' '.toolu/skills/**'` prints only lines in the `docs/runtime.md` migration note, which names `TOOLU_RUNTIME=bun` and `@toolu/core` as removed.
- **AC-11:** No `.github/workflows/*.yml` uses `oven-sh/setup-bun` or runs `bun`. `cargo xtask gate --only ci-paths --only workflows` passes on the new workflows, and fails on a fixture workflow that violates one ported rule, for example a missing `if` on a gated job.

- **AC-13:** Every shared golden that only the TypeScript harness replayed is replayed by a Rust test against the built `toolu`. That covers `gates/pre-tool-modules-{a,b,c}-golden.json` with their corpus rows, `gates/lifecycle-golden.json`, `quality/{ts,python,rust}-golden.json` and `statusline/cases.json`. Each test asserts its case count and names a mismatching case. A case it does not replay is listed with a reason. `toolu setup agents` keeps its former `setup.ts` parity as recorded transcripts.
- **AC-12:** With a `PATH` that holds no `bun` and no `~/.bun`, running every `SessionStart` and `UserPromptSubmit` hook entry of every plugin through the built binary in a temp config dir prints no text that contains `bun` (case-insensitive) on stdout or stderr. With the same `PATH`, `toolu doctor --json` reports no Bun check.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary / failure | Check |
|---|---|---|---|---|
| AC-1 | This branch's tree; `fixtures/guardrails/…`-style temp roots in `final_removal_test.rs` | Step prints `final-removal: ok` | A temp root with one `.ts`, a `bun.lock`, a `hooks/dist`, a `.sh` | `cargo test -p xtask final_removal`; `cargo xtask gate --only final-removal` |
| AC-2 | Real `plugins/*/hooks/hooks.json`; launcher fixture text files | Generated equals committed; no Bun tokens | Missing binary with Bun on `PATH` | `cargo test -p toolu-protocol launcher`; `cargo xtask check-hooks`; `cargo xtask launcher-e2e --bin target/debug/toolu` |
| AC-3 | Temp config dir with real `.js` and `.sh` modules and an installed-plugins file | Allowed; one advisory line | `.js` would deny; a `bun` shim on `PATH` that writes a marker file must stay unrun | `cargo test -p toolu-engine registry`; `cargo test -p toolu-cli --test registry_js` |
| AC-4 | Temp config dir with symlinks made by the previous code path, and a user file | Symlinks gone, user file intact | Foreign symlink kept | `cargo test -p toolu-jev -p toolu-statusline -p toolu-review session` |
| AC-5 | Temp config dir with the exact old bridge text | Only manifest left | Unrelated spec's `.sh` kept | `cargo test -p toolu-engine quality::plugin`; quality hook CLI tests |
| AC-6 | Repository | Gate `ok` | `PATH` filtered of every Bun dir | `env PATH=<filtered> cargo xtask gate` via job wrapper |
| AC-7 | `benchmarks/cases/hooks/payloads.json`, real hooks | Report JSON written, budgets pass | Temp budgets copy lowered → exit 1 | `cargo xtask bench-hooks --assert`; `cargo test -p xtask bench_hooks` |
| AC-8 | Workflows, config, `npm pack --dry-run` of `tools/toolu-cli/npm` | No core/conformance; exact file list | Wrapper without binary → 69 | `cargo xtask gate --only pack`; `cargo test -p toolu-cli --test npm_wrapper` |
| AC-9 | Live Claude Code and Codex sessions on temp git projects | Four denials observed; state unchanged | Bun hidden from `PATH` | `cargo xtask host-smoke --host both`; transcript paths recorded in `docs/toolu/evidence/2026-10-14-remove-typescript-and-bun.md`; CI `rust` job log |
| AC-10 | Repository Markdown | Gates pass; grep prints only the migration note | A skill naming `bun …/x.ts` fails (existing surfaces test) | `cargo xtask check-markdown-cli`; `cargo xtask gate --only docs-cli`; the AC's `git grep` |
| AC-11 | `.github/workflows/*.yml`; fixture workflows in `check_workflows_test.rs` | Pass; violation reported | One rule broken per fixture | `cargo test -p xtask check_workflows ci_yaml`; `cargo xtask gate --only ci-paths --only workflows` |
| AC-12 | Every plugin's `hooks.json` SessionStart/UserPromptSubmit entry, temp config dir | No Bun mention | `PATH` without Bun, `TOOLU_BUN` unset | `cargo test -p toolu-cli --test no_bun_advice` (spawns the generated launcher commands with `TOOLU_BIN`); `toolu doctor --json` in the same test |

| AC-13 | The committed goldens and the built binary | Every case replayed or listed with a reason | A corrupted golden value fails with the case name (checked by hand while porting) | `cargo test -p toolu-cli --test golden_gates --test golden_lifecycle --test golden_quality --test golden_statusline --test setup_agents_golden` |

## Documentation impact

- **Rewritten:** `docs/runtime.md`.
- **Updated:** `AGENTS.md`, `docs/testing.md`, `docs/registry.md` (executable protocol, `.js` advisory), `docs/resource-budgets.md` (bench commands), `docs/install.md`, and `tooling/templates/plugin-README.md`.
- **Plugin READMEs and settings docs:** `plugins/jev/README.md`, `plugins/statusline/README.md`, `plugins/toolu/settings/README.md` and `plugins/toolu/hooks/docs/push-review.md` lose their shim references.
- **Other docs:** `benchmarks/README.md` and `benchmarks/results/README.md`; `docs/gate-coverage-matrix.md` (host column); `docs/cli/` regenerated; `docs/portable-core.md` deleted; and every doc that names a deleted script.
- **Project skill:** `.toolu/skills/verify-dual-host-plugins/SKILL.md` names `cargo xtask host-smoke`.
- **Release notes:** `BREAKING CHANGE:` footers. Bun is no longer needed or used on Claude Code and Codex. `.js` registry modules no longer run. `@toolu/core` is deprecated (`npm deprecate` is a manual follow-up for the maintainer, listed in the PR). `TOOLU_RUNTIME=bun` is gone. The `jev.sh`, `statusline.sh` and `write-state.sh` paths are gone. Windows hooks report the missing binary.

## Open Questions

1. **Merge after a #425 release.** Owner: epic orchestrator or maintainer. Non-blocking for implementation. The PR states it.
2. **`npm deprecate @toolu/core`.** Owner: maintainer. It needs registry credentials the worker does not have. Non-blocking, and listed in the PR as a post-merge step.

## Deviations (execution)

- **Golden replays (AC-13).** Execution found about 600 golden cases (gates A/B/C, lifecycle, quality, statusline) whose only replayer was the TypeScript harness, through the `TOOLU_IMPL` seam. The Architecture's claim that every golden already had a Rust replayer was wrong for them. Jev chose to port them rather than drop the coverage (0.94). They are now Rust tests in `crates/cli/tests/golden_*.rs`. The statusline `jev`, `report` and `report-error` kinds (24 cases) describe the TypeScript plugin's text report and `jev.sh` probe, which #431 replaced by design, so they are counted, explained and not replayed.
- **`.sh`-free stable paths.** `toolu statusline refresh --json` reports `retired` instead of `published`.
- **YAML crate.** `serde_yaml_ng` failed `cargo deny` (unmaintained `unsafe-libyaml`) and `yaml-rust2` pulls the Zlib `foldhash`; `serde-saphyr` (deserialize only) passes. `rustls` moved to 0.23.46 for RUSTSEC-2026-0346, which fails `cargo deny` on main as well.
- **Interleave.** The state race of 8 TypeScript and 8 Rust writers lost its TypeScript half. A 16-writer Rust race tripped the 5 s lock timeout under load, and the fixture race already covers Rust writers, so it was dropped.

## Spec review (2026-10-14)

- Architecture: 🟡 should-fix (fixed). The `tooling/` deletion rule was vague. It now names the keep rule and the known survivors.
- Acceptance criteria: 🟡 should-fix (fixed). AC-10 was prose-only, and now has a runnable `git grep`. The issue scenario "nothing asks for Bun" had no AC, so AC-12 was added.
- Acceptance evidence: 🔵 consider. The issue asks for a live session on a clean Linux machine. AC-9 covers Linux through the `rust` CI job, which runs `launcher-e2e` and the hook tests with no Bun installed, rather than through a live Claude Code session. A live Linux host session needs host credentials inside a container. That limitation is recorded in the PR.
- Open Questions: both are owned and non-blocking.
- Jev (spec-review): coverage 1.62/2, observable 0.67, buildable 0.82, after the fixes above (it was 1.6, 0.54 and 0.79 before them).

**Status:** Approved
