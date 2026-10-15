# Remove the TypeScript implementation and the Bun prerequisite — Plan

**Date:** 2026-10-14   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-14-remove-typescript-and-bun-design.md   **Topic:** Delete the TypeScript tree, the Bun fallback and every Bun step; port what still runs to Rust.

## Evidence and approach

The plan rests on these inspected files:

- `crates/core/protocol/src/launcher.rs`: the Bun fallback, `TOOLU_RUNTIME` and the Bun-only `commandWindows`.
- `crates/core/engine/src/registry/{bridge.rs,bridge/,phase.rs,phase/batching.rs}`: the `.js` bridge.
- `crates/core/engine/src/quality/plugin.rs`: the `.sh` bridge.
- `crates/{jev,statusline,toolu-review}/src/session.rs`, `crates/statusline/src/refresh.rs` and `crates/jev/src/prompt.rs`: the shims.
- `crates/toolu/src/doctor/checks.rs`: the Bun check.
- `crates/xtask/src/{ci_yaml,check_workflows,dist,bench,package_workspace,portable_core,final_removal,gate_coverage*,pack,hooks_entries,bun_launcher,gate}.rs`: the Bun steps.
- The TypeScript tools that still run in CI: `tooling/src/benchmarks/{hook-resources,statusline}.ts`, `tooling/src/check-workflows.ts` and `tooling/src/ci-changes.ts`.
- `tooling/src/native-toolu-host-smoke.ts`: the live host smoke.
- The Rust tests that spawn Bun: `crates/cli/tests/helpers/{fallback,interop,resolution}.rs`, `ledger_{interop,leases}.rs`, `setup_agents_parity.rs`, `crates/core/engine/tests/{bridge,waiver_interop,pre_gates_interleave}.rs`, and the state parity helpers.

Recalled decisions:

- #425 kept the Bun fallback for one release. Memory 7a8ef399 records it.
- Long ledger checks need `PLAN_LEDGER_STEP_TIMEOUT=5400`, per memory 89c5389c.

The approach follows the approved spec. Edits run inline in the main thread: the work is one coherent removal, and its steps share files. Each step keeps the workspace compiling and its tests green, so every commit can be pushed.

## Workstream summary

1. Native product changes: launcher, registry, quality register, shims, doctor.
2. npm wrapper rewritten in JavaScript.
3. xtask loses Bun: YAML, the workflow check, step removals, `final-removal`.
4. New xtask tasks: hook bench, statusline bench, host smoke.
5. Mass deletion, plus the Rust parity tests de-Bunned.
6. Root toolchain, release config and CI.
7. Docs.
8. Bench results and live evidence.
9. Full gate without Bun.

## Steps (machine-readable)

```json
[
  {
    "id": "launcher",
    "title": "Launcher without Bun: native lookup or fail closed; commandWindows reports the missing binary; regenerate every hooks.json; xtask check-hooks accepts native entries only",
    "ac_refs": [
      "AC-2"
    ],
    "paths": [
      "crates/core/protocol/src",
      "crates/xtask/src",
      "crates/cli/tests/launcher.rs",
      "crates/cli/tests/helpers",
      "tooling/fixtures/native-launcher",
      "plugins/*/hooks/hooks.json"
    ],
    "input": "Real plugins/*/hooks/hooks.json; launcher golden text fixtures; a temp PATH holding a real bun and no toolu",
    "check": "cargo test -p toolu-protocol launcher && cargo test -p toolu-cli --test launcher missing_binary_blocks_even_with_bun && cargo test -p xtask hooks && cargo run -q -p xtask -- check-hooks && ! git grep -n -i -E 'TOOLU_RUNTIME|hooks/dist|bun' -- 'plugins/*/hooks/hooks.json' && cargo test -p toolu-cli --test launcher",
    "model": "inherit"
  },
  {
    "id": "registry",
    "title": "Registry: delete the Bun bridge; .js entries skipped as unsupported and no longer shadow .sh; SessionStart advisory per .js module",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "launcher"
    ],
    "paths": [
      "crates/core/engine/src/registry",
      "crates/core/engine/src/trace.rs",
      "crates/core/engine/tests",
      "crates/toolu/src/lifecycle",
      "crates/cli/tests/registry_js.rs"
    ],
    "input": "Temp config dir with x@t__deny.js, x@t__ok.sh and an installed-plugins file; a bun shim on PATH that writes a marker",
    "check": "cargo test -p toolu-engine && cargo test -p toolu-hub && cargo test -p toolu-cli --test registry_js",
    "model": "inherit"
  },
  {
    "id": "quality-register",
    "title": "Quality plugins register only the manifest and prune their old .sh bridge",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "registry"
    ],
    "paths": [
      "crates/core/engine/src/quality",
      "crates/ts-quality",
      "crates/python-quality",
      "crates/rust-quality",
      "crates/cli/tests/ts_quality_hook.rs",
      "crates/cli/tests/python_quality_hook.rs",
      "crates/cli/tests/rust_quality_hook.rs"
    ],
    "input": "Temp config dir holding the exact old bridge text and an unrelated spec's .sh",
    "check": "cargo test -p toolu-engine quality && cargo test -p toolu-ts-quality -p toolu-python-quality -p toolu-rust-quality && cargo test -p toolu-cli --test ts_quality_hook --test python_quality_hook --test rust_quality_hook",
    "model": "inherit"
  },
  {
    "id": "shims",
    "title": "Stop publishing jev.sh, statusline.sh, write-state.sh; remove toolu-owned symlinks, keep user files; jev context names toolu jev; doctor drops the Bun check; no hook output mentions Bun",
    "ac_refs": [
      "AC-4",
      "AC-12"
    ],
    "depends_on": [
      "quality-register"
    ],
    "paths": [
      "crates/jev",
      "crates/statusline",
      "crates/toolu-review",
      "crates/toolu/src",
      "crates/core/runtime/src/startup",
      "crates/cli/tests",
      "plugins/jev",
      "plugins/statusline",
      "plugins/toolu-review"
    ],
    "input": "Temp config dir with symlinks published by the previous code path, a regular user file, and a foreign symlink; PATH without bun",
    "check": "cargo test -p toolu-jev -p toolu-statusline -p toolu-review -p toolu-hub && cargo test -p toolu-cli --test no_bun_advice --test doctor",
    "model": "inherit"
  },
  {
    "id": "npm-wrapper",
    "title": "npm wrapper as committed dependency-free tools/toolu-cli/npm/cli.js with Rust integration tests; pack checks only @toolu/plugins",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "shims"
    ],
    "paths": [
      "tools/toolu-cli",
      "crates/cli/tests/npm_wrapper.rs",
      "crates/xtask/src/pack.rs",
      "crates/xtask/src/packaging_assets.rs"
    ],
    "input": "The built toolu binary; temp HOME/PATH layouts; npm pack --dry-run of tools/toolu-cli/npm",
    "check": "cargo test -p toolu-cli --test npm_wrapper && cargo test -p xtask pack && cargo run -q -p xtask -- gate --only pack",
    "model": "inherit"
  },
  {
    "id": "xtask-no-bun",
    "title": "xtask without Bun: Rust YAML in ci_yaml, check_workflows ported from TypeScript, dist check / workspace / portable-core steps and bench live/container removed, final-removal rewritten, gate-coverage native-only",
    "ac_refs": [
      "AC-1",
      "AC-11"
    ],
    "depends_on": [
      "npm-wrapper"
    ],
    "paths": [
      "crates/xtask",
      "Cargo.toml",
      "Cargo.lock",
      "deny.toml",
      "fixtures/gate-coverage",
      "docs/gate-coverage-matrix.md",
      "tooling/conventions/guardrails/rust/inventory.json",
      ".github"
    ],
    "input": "Real .github/workflows/*.yml; temp roots with one violation each (a .ts file, bun.lock, hooks/dist, a .sh, a gated job without if)",
    "check": "cargo test -p xtask && cargo test -p xtask final_removal && cargo test -p xtask check_workflows && cargo run -q -p xtask -- gate --only workflows --only ci-paths --only gate-coverage",
    "model": "inherit"
  },
  {
    "id": "bench-smoke",
    "title": "New xtask tasks bench-hooks, bench-statusline and host-smoke ported from the TypeScript tools",
    "ac_refs": [
      "AC-7",
      "AC-9"
    ],
    "depends_on": [
      "xtask-no-bun"
    ],
    "paths": [
      "crates/xtask",
      "benchmarks",
      "plugins/*/hooks/hooks.json",
      "tooling/conventions/guardrails/rust/inventory.json"
    ],
    "input": "benchmarks/cases/hooks/payloads.json and every hooks.json entry; a temp budgets copy lowered below the measurement",
    "check": "cargo test -p xtask bench && cargo test -p xtask bench_hooks_lowered_budget_fails && cargo test -p xtask host_smoke && cargo build --release --locked -p xtask -p toolu-cli && ./target/release/xtask bench-hooks --bin target/release/toolu --out /tmp/toolu-440-bench.json && ./target/release/xtask bench-statusline --bin target/release/toolu --assert",
    "model": "inherit"
  },
  {
    "id": "delete-ts",
    "title": "Delete packages/, tools/toolu-conformance, plugins hooks src/dist, TS tests and scripts, tooling TypeScript, fixtures/rust-ported.json; port delivery-flow contract tests to xtask; drop the live-TypeScript halves of Rust parity tests; drop deleted paths from .github/ci-paths.json so ci-paths stays green",
    "ac_refs": [
      "AC-1",
      "AC-6"
    ],
    "depends_on": [
      "bench-smoke"
    ],
    "paths": [
      "packages",
      "tools",
      "plugins",
      "tooling",
      "fixtures",
      "crates",
      "benchmarks",
      ".github/ci-paths.json"
    ],
    "input": "The repository tree; PATH filtered of every bun directory",
    "check": "test -z \"$(git ls-files '*.ts' '*.tsx' '*.mts' '*.cts' | grep -v '^benchmarks/fixtures/')\" && env PATH=\"$(printf %s \"$PATH\" | tr ':' '\\n' | grep -v -i bun | paste -sd: -)\" cargo test --workspace --locked && cargo run -q -p xtask -- gate --only ci-paths",
    "model": "inherit"
  },
  {
    "id": "golden-replay",
    "title": "Rust replays of the goldens the TypeScript harness owned: gates A/B/C with corpus rows, lifecycle, quality ts/python/rust, statusline; setup agents transcripts; delivery-flow Markdown contract",
    "ac_refs": [
      "AC-13",
      "AC-6"
    ],
    "depends_on": [
      "delete-ts"
    ],
    "paths": [
      "crates/cli/tests",
      "crates/xtask/tests/skills_contract.rs",
      "fixtures",
      "plugins"
    ],
    "input": "The committed goldens in fixtures/ and the built toolu",
    "check": "cargo test -p toolu-cli --test golden_gates --test golden_lifecycle --test golden_quality --test golden_statusline --test setup_agents_golden && cargo test -p xtask --test skills_contract",
    "model": "inherit"
  },
  {
    "id": "toolchain-ci",
    "title": "Minimal root package.json with package-lock.json (jscpd), bun.lock and TS configs deleted; release-please-config, npm-publish, release-native, tests.yml, toolu-review.yml and ci-paths.json without Bun",
    "ac_refs": [
      "AC-8",
      "AC-11",
      "AC-1"
    ],
    "depends_on": [
      "golden-replay"
    ],
    "paths": [
      "package.json",
      "package-lock.json",
      "release-please-config.json",
      ".github",
      "crates/xtask",
      "lefthook.yml"
    ],
    "input": "Real workflows and release config",
    "check": "! git grep -n -E 'setup-bun|(^|[^a-z])bun (run|install|test|x)( |$)|toolu-core|toolu-conformance' -- .github release-please-config.json package.json lefthook.yml && npm ci --ignore-scripts && cargo run -q -p xtask -- gate --only workflows --only ci-paths --only final-removal --only jscpd --only pack",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Docs: runtime.md rewritten; AGENTS.md, testing.md, registry.md, resource-budgets.md, install.md, plugin README template, plugin READMEs, benchmarks READMEs, verify-dual-host-plugins skill; docs/cli regenerated; portable-core.md deleted",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "toolchain-ci"
    ],
    "paths": [
      "AGENTS.md",
      "README.md",
      "docs",
      "tooling/templates",
      "plugins",
      "benchmarks",
      ".toolu/skills"
    ],
    "input": "Repository Markdown",
    "check": "cargo run -q -p xtask -- check-markdown-cli && cargo run -q -p xtask -- gate --only docs-cli --only context-budget && test -z \"$(git grep -n -E '(^|[^a-z])bun (run|install|test|build|x)( |$)|setup-bun|hooks/dist|TOOLU_RUNTIME|@toolu/core' -- AGENTS.md README.md ':(glob)docs/*.md' ':(glob)docs/cli/**' tooling/templates ':(glob)plugins/**/*.md' ':(glob).toolu/skills/**' | grep -v -E '^(docs/runtime.md|docs/conformance-report.md|docs/markdown-cli.md|docs/shell-analysis.md|plugins/toolu/hooks/docs/gates.md):' | grep -v 'deprecated')\"",
    "model": "inherit"
  },
  {
    "id": "evidence",
    "title": "Commit the native hook-bench result next to the Bun baselines; run the live host smoke without Bun and record the evidence",
    "ac_refs": [
      "AC-7",
      "AC-9"
    ],
    "depends_on": [
      "docs"
    ],
    "paths": [
      "benchmarks/results",
      "docs/toolu/evidence/2026-10-14-remove-typescript-and-bun.md"
    ],
    "input": "Real Claude Code and Codex sessions on temp git projects; the release binary",
    "check": "ls benchmarks/results/hook-resources-rust-darwin-arm64-2026-10-1*.json && grep -q 'host-smoke: ok' docs/toolu/evidence/2026-10-14-remove-typescript-and-bun.md",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full cargo xtask gate on a PATH without Bun",
    "ac_refs": [
      "AC-6",
      "AC-1",
      "AC-10",
      "AC-11"
    ],
    "depends_on": [
      "evidence"
    ],
    "paths": [
      "."
    ],
    "input": "The whole repository; PATH filtered of every bun directory",
    "check": "env PATH=\"$(printf %s \"$PATH\" | tr ':' '\\n' | grep -v -i bun | paste -sd: -)\" cargo xtask gate --base origin/main --title 'feat!: remove the TypeScript implementation and the Bun prerequisite (#440)'",
    "model": "inherit"
  }
]
```

## Deviations

- launcher: `cargo xtask launcher-e2e` requires the binary inside a fixed install directory, such as `/usr/local/bin`, which is root-owned on this machine. It is dropped from the local check and stays covered by the `rust` and `rust-macos` CI jobs, which install there with `sudo`.

- xtask-no-bun: the root toolchain and CI edits planned for `toolchain-ci` landed with this step, because the `ci-paths` and `workflows` checks need the workflows and `.github/ci-paths.json` to agree with the new jobs. `toolchain-ci` re-checks them.
- bench-smoke: the hook budgets are Linux CI numbers, and macOS has a higher process floor (about 3.4 ms CPU per spawn). So the local check runs `bench-hooks` without `--assert`; the Linux `hook-bench` CI leg asserts (AC-7). The statusline budget asserts locally.
- docs: the check's pathspecs are `:(glob)` so `docs/*.md` no longer descends into the historical `docs/toolu/` records, and the lines that legitimately name Bun are excluded: the migration notes (`runtime.md`), the historical conformance report, the markdown-cli gate's description of the old paths it rejects, the `bun test` examples of a user project's test command (`shell-analysis.md`, `gates.md`), and the `@toolu/core` deprecation notes.
- golden-replay (new step, AC-13): see the spec's Deviations. The gate, lifecycle, quality and statusline replays were delegated to implementers on sonnet, one per family, each writing only its own test files.

## Critical files

- `crates/core/protocol/src/{launcher,install}.rs` and their tests and fixtures; `plugins/*/hooks/hooks.json`
- `crates/core/engine/src/registry/**`, `crates/core/engine/src/trace.rs`, `crates/core/engine/src/quality/plugin.rs`
- `crates/toolu/src/lifecycle/**`, `crates/toolu/src/doctor/checks.rs`
- `crates/{jev,statusline,toolu-review}/src/**`
- `crates/xtask/src/{ci_yaml,check_workflows,final_removal,gate,main,bench,bench_hooks*,bench_statusline*,host_smoke*,pack,gate_coverage*,hooks_entries,check_hooks}.rs`; deleted: `bun_launcher.rs`, `package_workspace.rs`, `portable_core.rs`, and the `dist` drift half
- `crates/cli/tests/{registry_js,no_bun_advice,npm_wrapper}.rs`; `crates/xtask/tests/skills_contract.rs`
- `tools/toolu-cli/npm/{cli.js,package.json,README.md}`
- `package.json`, `package-lock.json`, `release-please-config.json`, `.github/workflows/*.yml`, `.github/ci-paths.json`
- Docs listed in the spec's Documentation impact

## Verification

- **End to end:** `cargo xtask host-smoke --host both` runs live on macOS with Bun hidden. It covers a denied `.env` write, a recorded quality failure, a denied commit and a denied push.
- **Gate:** `cargo xtask gate` passes on a `PATH` without Bun. CI's `rust` job has no Bun installed.
- **Failure boundaries:** a missing binary fails closed even with Bun present (AC-2); a `.js` module is never spawned (AC-3); a user file at a shim path survives (AC-4); `final-removal` fails on a re-added `.ts`, `bun.lock`, `hooks/dist` or `.sh` (AC-1); `bench-hooks --assert` fails on a lowered budget (AC-7); `check_workflows` fails on each broken rule (AC-11).
- **Delivery:** commit after each green step (`wip:` subjects allowed) and push the branch. Then run `toolu ledger run <plan> --verify` with `PLAN_LEDGER_STEP_TIMEOUT=5400`, `toolu-review:review` (version 2 state), and `toolu ledger verdict status` until it reports `overall: ready`. Then push, open the PR (title `feat!: remove the TypeScript implementation and the Bun prerequisite`; the body starts with `Closes Falconiere/toolu#440` and `Part of Falconiere/toolu#402`, states the merge-after-#425-release precondition, and ends with `BREAKING CHANGE:` footers), and hand off to `pr-babysit:babysit`.
- **Docs:** the `docs` step greps for Bun instructions and runs the Markdown-CLI and `docs-cli` gates.

## Plan review (2026-10-14)

- launcher: 🟡 should-fix (fixed). The missing-binary-with-Bun boundary had no named test. The check now runs `missing_binary_blocks_even_with_bun` and `launcher-e2e`, and `paths` declares the CLI launcher test and its helpers.
- bench-smoke: 🟡 should-fix (fixed). The lowered-budget failure is named (`bench_hooks_lowered_budget_fails`).
- xtask-no-bun: 🟡 should-fix (fixed). The `final_removal` and `check_workflows` boundary tests are named in the check.
- delete-ts: 🟡 should-fix (fixed). Deleting files that `.github/ci-paths.json` names would leave `ci-paths` red until `toolchain-ci`. The step now prunes those globs and runs `gate --only ci-paths`.
- evidence: 🔵 consider. The live host smoke costs model spend, so its check reads the recorded evidence rather than re-running sessions. The evidence file holds the command output and transcript paths.
- Ran the `docs` and `toolchain-ci` grep patterns: `\b` is unsupported by `git grep -E` on macOS, so they use explicit boundaries.
- Jev (plan-review): alignment 1.07/2, order 0.67, before the fixes above.

**Status:** Approved
