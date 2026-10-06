# toolu-runtime: host roots, config, settings, process, startup publish, registry types — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-toolu-runtime-design.md   **Topic:** #414. Port `@toolu/core/{host,config,process,startup,registry}` to `crates/core/runtime`.

## Evidence and approach

The spec decides the design; the brainstorm records the Jev calls. Port sources:
- `packages/toolu-core/src/host/{host-detect,host-name,host-roots,host-snapshot}.ts`;
- `packages/toolu-core/src/config/{config-files,config-load,config-read,gate-mode,quality-config,docs-sync-config,permissions,settings-dir,settings-files,settings}.ts`;
- `packages/toolu-core/src/process/{run-command,process-group}.ts`;
- `packages/toolu-core/src/startup/{publish,context,report,dependencies}.ts` and `state/state-io.ts` (`toJqJson`);
- `packages/toolu-core/src/registry/{registry-types,registry-paths}.ts`;
- `packages/toolu-core/src/cli/cli.ts` (`numberValue`).

Their `__tests__` are the source of the Rust unit cases. The existing crate holds `cli`, `install`, `invocation`, `manifest`, `namespace`, `skew` and `version`; `invocation.rs` already reads `std::env` here. `toolu-protocol` supplies `Host`, `HostEvent`, `Decision`, `NormalizedEvent`, `encode::supports_ask` and `native::hosts_for_native`. Integration tests follow `crates/core/protocol/tests` (`#[path = "helpers/repo.rs"]`, `Res<T>`); clippy allows unwrap and indexing in tests.

Feasibility probe (scratch crate): `std::env::home_dir()` compiles without a deprecation warning, and `nix 0.31` `killpg(pid, None)` and `killpg(pid, SIGKILL)` work on a `process_group(0)` child. `cargo` must be the rustup proxy (`$HOME/.cargo/bin`, toolchain 1.99.0 from `rust-toolchain.toml`); every check sets `PATH`.

Comemory `be52369e`: this host runs as root, so `chmod` cannot make a directory unwritable; no test relies on it (an unwritable path is made by putting a regular file where a directory must be).

The golden `fixtures/config/expected.json` is captured once by a scratch script outside the repository that imports the TypeScript loader, as #413 captured `fixtures/host/encode.json`; the TypeScript fixture test then keeps it honest.

## Workstream summary

Dependencies, env and JSON helpers → process → host and the root fixture → config loader (with the TypeScript `epic` key) → resolvers → the shared config golden (TypeScript and Rust) → permissions and settings → startup → registry types → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "foundation",
    "title": "nix signal feature, toolu-runtime deps (serde, nix), Env snapshot, JS-compatible JSON text (Number#toString, JSON.stringify compact/pretty, jq DEL escape, ordered top-level keys) and cli_args::jq_number, with unit tests",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t env::tests:: && t json::tests:: && t cli_args::tests::",
    "ac_refs": ["AC-1"],
    "paths": ["crates/core/runtime/", "Cargo.toml", "Cargo.lock"],
    "input": "env maps with empty and unset values, HOME unset; numbers 1, 1.0, 2.5, 1e21, 1e-7, -0, 12345678901234567890; strings with DEL and control characters; top-level texts with repeated and out-of-order keys; jq numbers ' 5 ', '+.5', '5.', '1e999', 'abc'",
    "model": "inherit"
  },
  {
    "id": "process",
    "title": "process::run (own process group, stdin thread, shared output budget, deadline with SIGTERM then SIGKILL, group-exit wait), signal_group/group_alive/terminate_group, git_toplevel and codex_plugin_list, with real-child tests",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t process::tests:: && t process::group::tests:: && t process::commands::tests::",
    "ac_refs": ["AC-7"],
    "depends_on": ["foundation"],
    "paths": ["crates/core/runtime/", "Cargo.toml", "Cargo.lock"],
    "input": "sh children: echo with exit 3; sleep past a 200 ms deadline with a backgrounded grandchild; a TERM-ignoring child; head -c 4096 /dev/zero under a 100-byte budget; a child that closes stdin before a large stdin write; a self-killed child (128+9); an empty argv; a real git init repo and a non-repo dir",
    "model": "inherit"
  },
  {
    "id": "host",
    "title": "host::detect, Roots (every path function, CallerError), the Codex plugin snapshot (canonicalize, atomic write, tri-state read) and the fixtures/host/root.json integration test",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t host::detect::tests:: && t host::roots::tests:: && t host::snapshot::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --test host_roots_fixture 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["process"],
    "paths": ["crates/core/runtime/", "fixtures/host/root.json", "Cargo.toml", "Cargo.lock"],
    "input": "fixtures/host/root.json (18 cases, real git sandboxes, GIT_CEILING_DIRECTORIES, unset HOME); host-detect and host-snapshot test cases ported from packages/toolu-core/src/host/__tests__ (a fake codex on PATH printing listings: object installed, disabled entries, non-JSON, exit 1); TOOLU_CONFIG_DIR with CODEX_HOME and PLUGIN_ROOT",
    "model": "inherit"
  },
  {
    "id": "config-load",
    "title": "config::load/exists/merge with the ordered envelope check and the epic key; the same epic key in the TypeScript schema with a colocated test; docs/config.md",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t config::load::tests:: && bun test packages/toolu-core/src/config/__tests__/config-load.test.ts packages/toolu-core/src/config/__tests__/config-schema.test.ts && grep -q '`epic`' docs/config.md",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["host"],
    "paths": ["crates/core/runtime/", "packages/toolu-core/src/config/", "fixtures/config/", "docs/config.md", "Cargo.toml", "Cargo.lock"],
    "input": "fixtures/config/*.json placed as user and project files in temp roots; truncated, empty, null, false, string-version and 1.0-version texts; a directory at the config path; invalid UTF-8; {\"version\":1,\"epic\":{\"anything\":true}} and the same with \"nope\"",
    "model": "inherit"
  },
  {
    "id": "config-resolve",
    "title": "Resolvers: section/enabled/flags/config_string/model/codex_model (config::read), gate_preset/gate_mode/gate_decision/guardrail_warning (config::gate_mode), quality thresholds and native_max_lines (config::quality), docs-sync globs (config::docs_sync)",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t config::read::tests:: && t config::gate_mode::tests:: && t config::quality::tests:: && t config::docs_sync::tests::",
    "ac_refs": ["AC-1"],
    "depends_on": ["config-load"],
    "paths": ["crates/core/runtime/", "fixtures/config/", "Cargo.toml", "Cargo.lock"],
    "input": "edge-values.json, edge-shapes.json, edge-legacy.json, edge-ask-all.json, docs-strict.json; real .oxlintrc.json/.eslintrc*/biome.json files in a temp root; an unknown gate name and an unknown model class",
    "model": "inherit"
  },
  {
    "id": "config-golden",
    "title": "Capture fixtures/config/expected.json from the TypeScript implementation; config-fixture.test.ts (TypeScript) and tests/config_fixture.rs (Rust) reproduce every case; fixtures/index.json, fixtures/README.md, fixtures/config/README.md, inventory test count",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --test config_fixture 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && bun test packages/toolu-core/src/config/__tests__/config-fixture.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts && bun run tooling/src/check-fixture-inventory.ts && grep -q 'expected.json' fixtures/README.md && grep -q 'expected.json' fixtures/config/README.md",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["config-resolve"],
    "paths": ["crates/core/runtime/", "fixtures/", "packages/toolu-core/src/config/", "tooling/src/check-fixture-inventory.ts", "tooling/src/__tests__/check-fixture-inventory.test.ts", "Cargo.toml", "Cargo.lock"],
    "input": "the 16 fixtures/config/*.json files and the inline boundary texts; boundary: a case count assertion so an empty file cannot pass, every fail-closed message, epic with and without an unknown top-level key",
    "model": "inherit"
  },
  {
    "id": "permissions-settings",
    "title": "config::permissions (one-time Claude allowlist write) and config::settings (settings_dir, read_list, list files, mcp_blocklist, code_edit_rules)",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t config::permissions::tests:: && t config::settings::tests::",
    "ac_refs": ["AC-9"],
    "depends_on": ["config-load"],
    "paths": ["crates/core/runtime/", "plugins/toolu/settings/", "Cargo.toml", "Cargo.lock"],
    "input": "real git repos in temp dirs; an existing settings.local.json with other keys, a malformed one (bytes must not change), allow as an object; the sentinel present; codex host; invalid config; autoAllow false; the shipped plugins/toolu/settings/*.txt and code-edit-rules.json",
    "model": "inherit"
  },
  {
    "id": "startup",
    "title": "startup::publish (symlink-only ownership, atomic relink), startup::report (TOOLU_STARTUP_REPORT records in TypeScript key order), startup::context (10 000 UTF-16 units, jq bytes), startup::dependencies (Codex warnings)",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t startup::publish::tests:: && t startup::report::tests:: && t startup::context::tests:: && t startup::dependencies::tests::",
    "ac_refs": ["AC-6"],
    "depends_on": ["host"],
    "paths": ["crates/core/runtime/", "Cargo.toml", "Cargo.lock"],
    "input": "a temp config root holding a user's regular file, a directory, a dangling symlink and a correct symlink at the publish path; a missing source; a regular file where the directory must go (unwritable on root); a report path inside a missing directory; text of 10 001 units ending in a surrogate pair; a fake codex on PATH with missing plugins",
    "model": "inherit"
  },
  {
    "id": "registry",
    "title": "registry: RegistryEvent, registry_root/event_dir, file_name/parse_name, strict ModuleManifest with read_manifest and matcher, the Rule trait and RuleContext",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t registry::tests:: && t registry::manifest::tests:: && t registry::rule::tests::",
    "ac_refs": ["AC-4", "AC-8"],
    "depends_on": ["host"],
    "paths": ["crates/core/runtime/", "Cargo.toml", "Cargo.lock"],
    "input": "manifest files in a temp pre-tools.d: valid, unknown field, version 2, name mismatch, event mismatch, empty matcher, matchers *, Bash|Edit, mcp__*; specs with whitespace, slash and __; TOOLU_CONFIG_DIR on Codex",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Crate //! doc and AGENTS.md Key files row for toolu-runtime; AC-5 dependency check",
    "check": "grep -q 'crates/core/runtime/src/lib.rs' AGENTS.md && grep -q '^//!.*config' crates/core/runtime/src/lib.rs && ! PATH=\"$HOME/.cargo/bin:$PATH\" cargo tree -p toolu-runtime -e normal --prefix none | grep -Eq '^(tree-sitter|brush|rustls|ureq)'",
    "ac_refs": ["AC-5"],
    "depends_on": ["config-golden", "permissions-settings", "startup", "registry"],
    "paths": ["AGENTS.md", "crates/core/runtime/src/lib.rs", "Cargo.toml", "Cargo.lock"],
    "input": "the final crate and lockfile",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full Rust gate and the TypeScript checks this change touches",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main && bun run tooling/src/check-fixture-inventory.ts && bun test packages/toolu-core/src/config/__tests__ packages/toolu-core/src/host/__tests__ tooling/src/__tests__/check-fixture-inventory.test.ts",
    "ac_refs": ["AC-9"],
    "depends_on": ["docs"],
    "input": "the whole branch; boundary: coverage floor 85% for toolu-runtime, zero jscpd clones, unused pub items, a fixture case missing from index.json",
    "model": "inherit"
  }
]
```

## Critical files

- Create, each module with its `tests/<module>_test.rs` beside it:
  - `crates/core/runtime/src/{env,json,cli_args,host,config,process,startup,registry}.rs`;
  - `src/host/{detect,roots,snapshot}.rs`;
  - `src/config/{files,load,read,gate_mode,quality,docs_sync,permissions,settings}.rs`;
  - `src/process/{group,commands}.rs`;
  - `src/startup/{publish,report,context,dependencies}.rs`;
  - `src/registry/{manifest,rule}.rs`;
  - `crates/core/runtime/tests/{host_roots_fixture,config_fixture}.rs` and `tests/helpers/repo.rs`;
  - `fixtures/config/expected.json` and `packages/toolu-core/src/config/__tests__/config-fixture.test.ts`.
- Modify:
  - `crates/core/runtime/src/lib.rs`, `crates/core/runtime/Cargo.toml`, root `Cargo.toml` (`nix` feature `signal`), `Cargo.lock`;
  - `packages/toolu-core/src/config/config-schema.ts` and its test (`epic`);
  - `docs/config.md`, `fixtures/index.json`, `fixtures/README.md`, `fixtures/config/README.md`, `tooling/src/__tests__/check-fixture-inventory.test.ts`, `AGENTS.md`.

## Verification

`cargo xtask gate --base origin/main` is the whole Rust bar: fmt, clippy, guardrails (file and impl sizes, test layout, folders), layers and capabilities (`std::process::Command` only under `src/process`), coverage at 85%, jscpd, unused pub, docs-cli and cli-compat. Then `bun test` over the config and host suites, `check-fixture-inventory`, and `bun run test`, whose known environment failures on this root host (comemory `be52369e`) are compared against a clean `origin/main` worktree before any is taken as a regression.

Real inputs are the committed fixtures, real `git` repositories, real child processes and real files and symlinks in temp directories. Docs synced: `docs/config.md`, `fixtures/README.md`, `fixtures/config/README.md`, `fixtures/index.json`, `AGENTS.md`, the crate doc.

## Delivery

1. Scoped commits on `feat/414-toolu-runtime-host-roots-config` (conventional subjects; the PR is squash-merged).
2. `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run docs/toolu/plans/2026-10-06-toolu-runtime.md --verify` over the whole branch diff.
3. `toolu-review:review`, recording the version 2 push-review state.
4. `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/verdict.js" status` must report `overall: ready`.
5. Fetch and rebase on `origin/main` if it moved; re-run the affected checks.
6. Push and open the PR against `main`: title `feat(runtime): host roots, config, settings, process, startup publish, registry types (#414)`, body starting with `Closes Falconiere/toolu#414` and `Part of Falconiere/toolu#402`.
7. Report `pr-open`, then `babysit`, and hand off to `pr-babysit:babysit`.

`TOOLU_PLUGIN_ROOT` is the enabled user install of `toolu@toolu` holding `plan-ledger.js` and `verdict.js`.

## Plan review

- **Round 1.** AC coverage checked by hand: AC-1 to AC-9 each have at least one `ac_refs` step and no ref dangles. Jev: boundary inputs on every behavior step 0.88; every AC has a step whose check fails when the AC breaks 0.69. The weak link is AC-9's `bun run test`, which this root host cannot pass whole (comemory `be52369e`).
  - gate: 🔵 consider: the ledger check runs the targeted Bun suites, not the whole `bun run test`. Kept: the whole suite runs in Verification against a clean `origin/main` baseline, and CI's `typescript` job is the authority.
  - Filters: each `--lib` filter is an anchored module path (`process::tests::` is not a substring of `process::group::tests::`), and the helper requires at least one passing test.
- **Status:** Approved.

## Deviations

Interfaces settled during execution; behaviour is the spec's:
- `config::load::load(roots, cwd)` and `exists(roots, cwd)` take a `Roots`, not `ConfigOptions`: the binding carries the one host-detection warning, which the caller prints without the `toolu-config: ` prefix. `LoadedConfig::from_data` builds a config a host merged itself (and the resolver tests).
- `json::ordered::Ordered` was added: an insertion-ordered JSON value. The permissions write rewrites the user's `settings.local.json` and the SessionStart context prints `hookEventName` before `additionalContext`, both in TypeScript's key order, which `serde_json::Map` would sort. Values printed through a `serde_json::Value` (a `version` object in a message, an object inside `docsSync`) still print sorted, as the spec's known divergences say.
- `config::settings::read_list` returns `Result`: TypeScript throws on an unreadable list, which fails the hook closed; an empty list would fail open.
- `quality_threshold(config, lang, key, root)`: `root` `None` is the git toplevel of the process directory, as TypeScript's default.
- `docs_sync_globs(config, DocsSyncKey)` replaces the three named functions; `settings_dir(roots, plugin_root)`; `permissions_autowrite(config, env, root, cwd)`; `host::snapshot::codex_plugin_snapshot_path`.
