# Native hook launcher — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-native-hook-launcher-design.md   **Topic:** Issue #412, generated native launcher, minimal `toolu` binary, skew rule, xtask print/check/e2e

## Evidence and approach

The work builds on what is already there:

- **Today's launcher and gate.** The Bun launcher (`packages/toolu-core/src/launcher/launcher.ts`) and its gate (`tooling/src/check-hooks-json.ts`) are the templates for messages and checks.
- **The empty core crates** under `crates/core/*` take the protocol and runtime pieces.
- **`crates/xtask`** supplies the task table (`TASKS`), `Options`, the `output` module, the `gate` step list and the behaviour inventory (`tooling/conventions/guardrails/rust/inventory.json`).
- **The layer and capability rules** (`rules.json`): stdout and stderr only in `toolu-protocol`, `toolu-cli::output` and `xtask::output`; env only in `toolu-runtime` and `xtask`; `process::Command` only in `toolu-runtime::process` and `xtask`.
- **#409's argv** (`tools/toolu-conformance/src/harness/entry-command.ts`).

Verified inputs:

- Codex 0.160.0 accepts `hookProtocol`: an isolated `CODEX_HOME`, then `codex plugin marketplace add <repo copy>` and `codex plugin add toolu@toolu`, installed and enabled the plugin. `claude plugin validate <copy>` passes with it.
- The npm wrapper builds with `bun build --target=node` and rejects `--hook-protocol` with exit 2.
- Codex and Claude both read `timeout` in seconds.

The `crates/cli` tests run the real binary (`CARGO_BIN_EXE_toolu`) through the real generated command with `sh -c`. They use temporary `HOME` and plugin roots, the real Bun, and the real npm wrapper, with shell stand-ins only for the signal cases (AC-8). The xtask `launcher_e2e` tests judge the checker's own exit 0/1/2 logic with an executable stand-in under a temporary `HOME`. The real binary goes through `launcher-e2e` in the `e2e-local` step (this host, linuxbrew and `/usr/local/bin`) and in CI. That meets the spec's AC-4 evidence row.

## Workstream summary

1. protocol generator
2. runtime manifest and skew
3. the `toolu` binary
4. launcher black-box tests
5. xtask tasks and gate step
6. manifests
7. TypeScript gate skip
8. CI
9. docs
10. local end-to-end proxy
11. full gate

## Steps (machine-readable)

```json
[
  {
    "id": "protocol",
    "title": "toolu-protocol: HOOK_PROTOCOL, enforcing events, install commands, cmd-safe messages, launcher generator (POSIX command, commandWindows, timeout) with name/event validation, read_stdin; co-located unit tests incl. committed goldens; Cargo.lock refreshed for the serde_json edge before --locked",
    "ac_refs": [
      "AC-1",
      "AC-3"
    ],
    "paths": [
      "crates/core/protocol/**",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "Targets (toolu, PreToolUse, pre-tools), (toolu, SessionStart, session-start), (jev, SessionStart, session-start); invalid names Bad_Name, empty, 'a/b', event 'preToolUse'; golden files crates/core/protocol/src/tests/fixtures/launcher-{pre-tool-use,session-start}.txt; cmd-unsafe character scan of every message",
    "check": "cargo test -p toolu-protocol --locked && cargo xtask gate --only fmt --only clippy --only guardrails --only layers --only reach --only unused-pub --only jscpd",
    "model": "inherit"
  },
  {
    "id": "runtime",
    "title": "toolu-runtime: manifest::read (.claude-plugin then .codex-plugin, strict hookProtocol), version compare, skew::assess with literal advisory/mismatch texts (incl. P<n over integers), install::upgrade_command (canonical path, Cellar/opt-homebrew/linuxbrew), invocation::{args,current_exe}; co-located unit tests",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "protocol"
    ],
    "paths": [
      "crates/core/runtime/**",
      "crates/core/protocol/**",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "Real plugin.json files written to temp dirs: valid, malformed, hookProtocol 0/\"1\"/1.0/absent/-1/2^32, missing .claude-plugin with .codex-plugin present, malformed .claude-plugin with valid .codex-plugin, empty root path; versions 7.10.0 vs 7.11.0, 7.9.0, 8.0.0, 7.10.0-rc.1, 7.10.0+b, garbage; binary paths through a temp Cellar symlink, /opt/homebrew/bin/toolu, /usr/local/bin/toolu; skew with n=2,P=1",
    "check": "cargo test -p toolu-runtime --locked && cargo xtask gate --only fmt --only clippy --only guardrails --only layers --only reach --only unused-pub --only jscpd",
    "model": "inherit"
  },
  {
    "id": "cli",
    "title": "crates/cli (package toolu-cli, bin toolu): argv parsing, --version, --hook-protocol, [<plugin>] hook <name> --event --plugin-root, skew prelude, output composition, native toolu session-start, unknown-hook outcome, exit 64 usage; register in Cargo.toml members and folders.json; unit tests in src/tests own their fixtures and reach the 85% floor alone",
    "ac_refs": [
      "AC-11",
      "AC-5"
    ],
    "depends_on": [
      "runtime"
    ],
    "paths": [
      "crates/cli/**",
      "crates/core/**",
      "Cargo.toml",
      "Cargo.lock",
      "tooling/conventions/guardrails/rust/folders.json"
    ],
    "input": "The built binary with --version, --hook-protocol, --nope, no argv; hook session-start with startup/resume/clear/empty/non-JSON stdin; unknown hook under PreToolUse and SessionStart and with no --event; --plugin-root absent, empty string, and temp plugin roots written by the cli tests themselves",
    "check": "cargo test -p toolu-cli --locked --bins && cargo xtask gate --only fmt --only clippy --only guardrails --only layers --only reach --only unused-pub --only jscpd",
    "model": "inherit"
  },
  {
    "id": "launcher-tests",
    "title": "Black-box launcher tests in crates/cli/tests (helpers forward LLVM_PROFILE_FILE through env -i so coverage counts): missing (schema-checked SessionStart JSON by a dependency-free helper reading the vendored schema), native diagnostic from ~/.local/bin under PATH=/usr/bin:/bin, skew cases incl. Cellar symlink and major skew, TOOLU_BIN non-executable, real npm wrapper first on PATH, SIGABRT/SIGSEGV stand-ins, Bun fallback with a real bundle exiting 3; fixed-install-dir precondition check",
    "ac_refs": [
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9"
    ],
    "depends_on": [
      "cli"
    ],
    "paths": [
      "crates/cli/**",
      "crates/core/**",
      "Cargo.toml",
      "Cargo.lock",
      "tooling/fixtures/codex-hook-schemas/session-start.command.output.schema.json",
      "tools/toolu-cli/src/**",
      "tools/toolu-cli/package.json",
      "tools/toolu-opencode/src/**",
      "bun.lock"
    ],
    "input": "Real toolu binary copied into temp HOME/.local/bin; real Bun on PATH (after bun install --frozen-lockfile); real npm wrapper built from tools/toolu-cli/src/cli.ts with bun build --target=node; temp plugin roots with edited version/hookProtocol; vendored Codex session-start schema; shell stand-ins that kill -ABRT/-SEGV themselves",
    "check": "cargo test -p toolu-cli --locked --test launcher && cargo xtask gate --only fmt --only clippy --only guardrails --only layers --only reach --only unused-pub --only jscpd",
    "model": "inherit"
  },
  {
    "id": "manifests",
    "title": "Add \"hookProtocol\": 1 to every plugins/*/.claude-plugin/plugin.json and .codex-plugin/plugin.json; packaging validation stays green",
    "ac_refs": [
      "AC-1"
    ],
    "paths": [
      "plugins/*/.claude-plugin/plugin.json",
      "plugins/*/.codex-plugin/plugin.json",
      "tooling/src/validate-plugin-packaging.ts",
      ".claude-plugin/marketplace.json",
      ".agents/plugins/marketplace.json",
      "release-please-config.json"
    ],
    "input": "All committed plugin manifests",
    "check": "bun run tooling/src/validate-plugin-packaging.ts && bun -e 'const g=new Bun.Glob(\"plugins/*/.{claude,codex}-plugin/plugin.json\");let n=0;for(const f of g.scanSync(\".\")){const j=await Bun.file(f).json();if(j.hookProtocol!==1)throw new Error(f);n++}if(n<20)throw new Error(\"only \"+n)' && claude plugin validate plugins/toolu",
    "model": "inherit"
  },
  {
    "id": "xtask",
    "title": "xtask print-hook, check-hooks (split into modules: native detection and name parse, entry compare and timeout, manifest hookProtocol) and launcher-e2e (fixed-dir setup rules, env -i run, canonical path compare); options --timeout/--bin; gate step `hooks` (STEPS, gate_test.rs count, USAGE, tests.yml step comment); inventory entries with pass and fail tests; xtask depends on toolu-protocol (Cargo.lock refreshed)",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-4",
      "AC-11"
    ],
    "depends_on": [
      "protocol",
      "manifests"
    ],
    "paths": [
      "crates/xtask/**",
      "crates/core/protocol/**",
      "Cargo.toml",
      "Cargo.lock",
      "tooling/conventions/guardrails/rust/inventory.json",
      "plugins/*/hooks/hooks.json",
      "plugins/*/.claude-plugin/plugin.json",
      "plugins/*/.codex-plugin/plugin.json"
    ],
    "input": "The repository plugins/ tree; temp copies with a native entry from print-hook, a hand-edited one, one without timeout, timeout 0 and 601, an unparsable name, a manifest without hookProtocol or with 2, versions bumped to 7.11.0 and 8.0.0; print-hook with --timeout 0, 601, abc, name Bad_Name, unknown plugin; launcher-e2e with a temp HOME whose ~/.local/bin/toolu is an executable stand-in answering --hook-protocol/--version/hook (exit 0 when it reports its own path, exit 1 when it reports another), and --bin outside the fixed dirs (exit 2). The real binary through launcher-e2e is the gate-local and CI steps",
    "check": "cargo test -p xtask --locked && cargo xtask check-hooks && cargo xtask gate --only fmt --only clippy --only guardrails --only layers --only reach --only unused-pub --only jscpd",
    "model": "inherit"
  },
  {
    "id": "ts-gate",
    "title": "tooling/src/check-hooks-json.ts recognises native entries (--hook-protocol) before isLauncherHook and skips them; test builds its native fixture from the committed Rust golden launcher-pre-tool-use.txt",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "protocol"
    ],
    "paths": [
      "tooling/src/check-hooks-json.ts",
      "tooling/src/__tests__/check-hooks-json.test.ts",
      "crates/core/protocol/src/tests/fixtures/launcher-pre-tool-use.txt",
      "plugins/*/hooks/hooks.json",
      "packages/toolu-core/src/launcher/**"
    ],
    "input": "Temp repo with plugins/x/hooks/hooks.json holding the golden native entry plus one generated Bun entry and its bundle; then the Bun entry hand-edited (must fail); the real repository",
    "check": "bun test tooling/src/__tests__/check-hooks-json.test.ts && bun run check:hooks-json",
    "model": "inherit"
  },
  {
    "id": "ci",
    "title": "CI: rust group globs for plugins/*/hooks/hooks.json and plugin manifests (changes.test.ts expectations for jev plugin.json edit/delete flip rust to \"true\"; new hooks.json case); tests.yml rust job launcher-e2e step installing the release binary into the Homebrew bin dir and /usr/local/bin per OS with cleanup; rust-musl static check on toolu; workflows.test.ts asserts both",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "xtask"
    ],
    "paths": [
      ".github/ci-paths.json",
      ".github/workflows/tests.yml",
      "tooling/src/ci-changes.ts",
      "tooling/src/check-ci-paths.ts",
      "tooling/src/ci-paths/**"
    ],
    "input": "The real workflow YAML and path data; seeded repos editing plugins/jev/.claude-plugin/plugin.json, deleting plugins/jev/.codex-plugin/plugin.json, editing plugins/toolu/hooks/hooks.json",
    "check": "bun run check:ci-paths && bun test tooling/src/ci-paths/__tests__",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/install.md (launcher resolution, signature, messages, fallback, TOOLU_BIN, hang gap until #413, running tests with toolu installed), docs/runtime.md 'Native launcher' section, AGENTS.md key files, xtask row, contributing line, CI table and rust group; regenerate the OpenCode surface copies (bun run generate:opencode-surface)",
    "ac_refs": [
      "AC-1",
      "AC-4"
    ],
    "depends_on": [
      "xtask",
      "ci"
    ],
    "paths": [
      "docs/install.md",
      "docs/runtime.md",
      "AGENTS.md",
      "tools/toolu-opencode/generated/**",
      ".github/ci-paths.json"
    ],
    "input": "The implemented behaviour and command names",
    "check": "grep -q TOOLU_BIN docs/install.md && grep -q '#413' docs/install.md && grep -q 'Native launcher' docs/runtime.md && grep -q launcher-e2e AGENTS.md && grep -q check-hooks AGENTS.md && grep -q print-hook docs/runtime.md && bun run test:docs",
    "model": "inherit"
  },
  {
    "id": "e2e-local",
    "title": "Local proxy of the CI leg on this root Linux host: release-build toolu, install it into /home/linuxbrew/.linuxbrew/bin then /usr/local/bin in turn, run cargo xtask launcher-e2e for each, remove it",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "runtime",
      "cli",
      "xtask"
    ],
    "paths": [
      "crates/**",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "The release toolu binary at the real Homebrew (linuxbrew) and installer directories, PATH=/usr/bin:/bin",
    "check": "cargo build --release -p toolu-cli --locked && for d in /home/linuxbrew/.linuxbrew/bin /usr/local/bin; do mkdir -p \"$d\" && install -m 0755 target/release/toolu \"$d/toolu\" && cargo xtask launcher-e2e --bin \"$d/toolu\"; s=$?; rm -f \"$d/toolu\"; [ $s -eq 0 ] || exit $s; done; rmdir /home/linuxbrew/.linuxbrew/bin /home/linuxbrew/.linuxbrew /home/linuxbrew 2>/dev/null; true",
    "model": "inherit"
  },
  {
    "id": "unit-baseline",
    "title": "bun run test:unit fails no test that a clean origin/main worktree passes on this host (environmental failures, comemory be52369e/9cda9086; base-only load flakes such as the detect import-cost test are tolerated); CI runs the full suite",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "ts-gate"
    ],
    "paths": [
      "tooling/**",
      "packages/**",
      "plugins/**",
      "tools/**",
      "bun.lock",
      "package.json"
    ],
    "input": "The branch and a detached origin/main worktree, both running bun run test:unit on this host",
    "check": "B=$(mktemp -d) && git worktree add -q --detach \"$B/base\" origin/main && (cd \"$B/base\" && bun install --frozen-lockfile >/dev/null 2>&1 && bun run test:unit > \"$B/base.log\" 2>&1); bun run test:unit > \"$B/branch.log\" 2>&1; for f in base branch; do grep '^(fail)' \"$B/$f.log\" | sed 's/ \\[[0-9.]*ms\\]//' | sort > \"$B/$f.txt\"; done; grep -q ' pass' \"$B/base.log\" && test -z \"$(comm -13 \"$B/base.txt\" \"$B/branch.txt\")\"; s=$?; git worktree remove --force \"$B/base\"; rm -rf \"$B\"; exit $s",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gates after bun install --frozen-lockfile: cargo xtask gate (incl. tests and coverage floors) and every bun run test (test:ts) script except test:unit, which unit-baseline judges",
    "ac_refs": [
      "AC-11"
    ],
    "depends_on": [
      "protocol",
      "runtime",
      "cli",
      "launcher-tests",
      "manifests",
      "xtask",
      "ts-gate",
      "ci",
      "docs",
      "e2e-local",
      "unit-baseline"
    ],
    "paths": [
      "crates/**",
      "Cargo.toml",
      "Cargo.lock",
      "plugins/**",
      "tooling/**",
      "docs/**",
      "AGENTS.md",
      ".github/**",
      "tools/**",
      "packages/**",
      "bun.lock",
      "package.json"
    ],
    "input": "The whole branch",
    "check": "bun install --frozen-lockfile && cargo xtask gate && bun run test:conventions && bun run test:portable-core && bun run test:gate-coverage && bun run test:final-removal && bun run check:ci-paths && bun run check:plugin-bundles && bun run check:hooks-json && bun run test:workspace && bun run test:pack && bun run test:conformance && bun run test:context-budget && bun run benchmarks --tier deterministic && bun run bench:shell --assert",
    "model": "inherit"
  }
]
```

## Deviations

- **launcher-tests:** one integration-test crate, `crates/cli/tests/launcher.rs`, with case modules `tests/helpers/{missing,native,skew,resolution,crash,fallback}.rs` and a shared `tests/helpers/sandbox.rs`. It replaces six `launcher_*` crates. Warnings are denied, so a shared helper included by six crates would be dead code in every crate that skips one of its items. The cases and ACs are unchanged; the check runs `--test launcher`.
- **launcher-tests:** the sandbox serialises writes of executables against child spawns (`RwLock`). A child forked while a copied binary was still open for writing made the launcher's probe fail with `ETXTBSY` on Linux, which showed up as one flaky run in about 25.

- **gate:** `bun run test:unit` fails 18 tests on this root host, and a clean `origin/main` worktree at the same base fails exactly the same 18: root ignores `chmod`, merged `/bin` and `/usr/bin` on `PATH`, ownership checks (comemory `be52369e`, `9cda9086`). So `gate` runs every other `test:ts` script, and the new `unit-baseline` step requires every `test:unit` failure on the branch to fail on `origin/main` too. A base-only failure, such as the load-sensitive detect import-cost test (comemory `38d27723`), is not a branch regression. CI's full `bun run test` stays the authority (Jev 0.77 that this hides no branch-caused failure).

## Delivery

- **Commits.** Commit after each green step with `feat(launcher): …` subjects; the PR squash-merges. Push once the review state allows it.
- **Final checks.** Before the PR, run `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-05-native-hook-launcher.md --verify`, then `toolu-review:review`. `verdict.js status` must report `overall: ready`.
- **The PR.** Title `feat(launcher): native hook launcher finds the installed toolu (#412)`. The body starts `Closes Falconiere/toolu#412` / `Part of Falconiere/toolu#402`, names `cargo xtask launcher-e2e` as the check for #457's real installs, and records the narrowing in Non-Goal 5.
- **Babysit.** The Homebrew and `/usr/local/bin` legs run only in the CI `rust` job on Linux and macOS, so babysit watches them.
- **Lock files.** Tests that build the npm wrapper need `bun install --frozen-lockfile`. Cargo commands use `--locked` after the lock is refreshed in the step that adds a dependency edge.

## Critical files

- **Create:**
  - `crates/core/protocol/src/{launcher,messages,event}.rs` and their tests, with goldens under `src/tests/fixtures/`;
  - `crates/core/runtime/src/{manifest,skew,version,install,invocation}.rs` and tests;
  - `crates/cli/{Cargo.toml,src/main.rs,src/hook.rs,src/session_start.rs,src/output.rs}`, `src/tests/*`, and `tests/launcher_*.rs` plus `tests/helpers/`;
  - `crates/xtask/src/{print_hook,check_hooks,launcher_e2e}.rs` and tests;
  - `tooling/src/__tests__/check-hooks-json.test.ts`, unless an existing test file covers it.
- **Modify:**
  - `Cargo.toml`, `Cargo.lock`;
  - `crates/core/{protocol,runtime}/{Cargo.toml,src/lib.rs}`;
  - `crates/xtask/{Cargo.toml,src/main.rs,src/options.rs,src/gate.rs}`;
  - `tooling/conventions/guardrails/rust/{folders,inventory}.json`;
  - every `plugins/*/.{claude,codex}-plugin/plugin.json`;
  - `tooling/src/check-hooks-json.ts`;
  - `.github/ci-paths.json`, `.github/workflows/tests.yml`;
  - `docs/install.md`, `docs/runtime.md`, `AGENTS.md`.

## Verification

End to end, the generated launcher drives the real binary from `~/.local/bin` under a reduced `PATH` in the `crates/cli` tests. `e2e-local` and CI repeat this through `cargo xtask launcher-e2e` at the Homebrew directory and `/usr/local/bin`, Linux locally and Linux plus macOS in CI.

Failure and boundary coverage:

- missing binary;
- bad `TOOLU_BIN`;
- wrapper shadowing;
- signal deaths;
- every skew row;
- malformed manifests;
- `--plugin-root ""`;
- out-of-range timeouts and names.

Each has a test. Docs sync is checked by `bun run test:docs`. The gate runs as `cargo xtask gate` and `bun run test`. Failures that are environmental on this root host (comemory `be52369e`) are baselined against a clean `origin/main` worktree before being judged.
