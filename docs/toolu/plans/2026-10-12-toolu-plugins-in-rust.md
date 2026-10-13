# `toolu plugins`: the installer in Rust — Plan

**Date:** 2026-10-12   **Status:** Draft   **Spec:** docs/toolu/specs/2026-10-12-toolu-plugins-in-rust-design.md   **Topic:** #438: port `tools/toolu-cli` to `crates/plugins` as `toolu plugins`, behind a thin `@toolu/plugins` wrapper

## Evidence and approach

- Inspected the whole TS installer (`tools/toolu-cli/src`: args, catalog,
  config, host, plugins, ui, opencode) and its tests; `crates/cli/src/registry.rs`
  (the `PLUGINS` placeholder), `crates/cli/src/dispatch.rs` (`Ctx`, `finish`),
  `toolu_runtime::{cli, env::Env, process::run, atomic, git, host::roots}`,
  `toolu_protocol::exit::Exit`, `crates/core/protocol/src/launcher.rs`
  (`INSTALL_DIRS`), `crates/statusline/Cargo.toml` (plugin crate shape),
  `tooling/conventions/guardrails/rust/{rules,folders,layers}.json`,
  `tooling/src/opencode-host/scenarios-cli.ts`, `crates/xtask/src/{pack,package_workspace}.rs`,
  `tooling/src/{check-workspace,pack-inventory}.ts`, the npm publish and npx tests,
  and `jsonc-parser` 0.34 (scanner tokens, `parse_to_ast` ranges, CST).
- Recalled: docs/toolu is gitignored (add with `-f`); Rust test helpers return
  `Result`; local gate env quirks (`TMPDIR=/private/tmp`, `TOOLU_BIN=target/debug/toolu`);
  no inline `bun -e`.
- Approach: one module per TS module, each with `tests/<module>_test.rs`;
  binary-level tests in `crates/cli/tests/plugins_*.rs` (only `toolu-cli` has
  the bin); the TS OpenCode subprocess suite stays in Bun and spawns the built
  binary through `requiredBuiltTooluBinary`. The external host CLIs are
  stubbed only to drive failure paths (AC-3, AC-4), as the TS suite did.

- New-crate registration (AGENTS.md Contributing): root `Cargo.toml`
  `members` and `[workspace.dependencies] jsonc-parser`; `crates = [... "plugins"]`
  in `tooling/conventions/guardrails/rust/folders.json`; `version.workspace`,
  `[lints] workspace = true`, crate `//!` doc, `crates/plugins/README.md`; no
  layer row (a plugin-layer crate links only core); no `inventory.json` row
  (not an xtask task); `coverage-floor.json` only if the default floor needs a
  row; `cargo deny check` for the new dependency.

## Workstream summary

crate and catalog → claude/codex verbs → flags, replay, prompts → OpenCode →
wrapper and TS removal with the Bun suite on the binary → smoke and tooling →
Markdown and notices → docs and generated reference → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "crates/plugins scaffold: clap tree, embedded catalog, dependency order, registry wiring",
    "check": "cargo test -p toolu-plugins -- catalog order && cargo build -p toolu-cli && ./target/debug/toolu plugins --help | grep -q install && cargo xtask gate --only deny --only guardrails --only layers",
    "ac_refs": ["AC-1"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates/plugins", "crates/cli/Cargo.toml", "crates/cli/src/registry.rs", "tooling/conventions/guardrails/rust/folders.json", ".claude-plugin/marketplace.json", "deny.toml"],
    "input": "the real .claude-plugin/marketplace.json; a catalog with a cycle; an unknown name",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Claude Code and Codex: adapters, detection, install/list/remove/update, text and JSON reports",
    "check": "cargo test -p toolu-plugins -- host verbs report run_step && cargo test -p toolu-cli --test plugins_hosts",
    "ac_refs": ["AC-3", "AC-9", "AC-12"],
    "depends_on": ["S1"],
    "paths": ["crates/plugins/src", "crates/cli/tests/plugins_hosts.rs", "crates/cli/tests/helpers"],
    "input": "a temp PATH whose claude script fails only `plugin install toolu@toolu`, logs argv and prints recorded `plugin list --json` payloads; a codex payload `{installed:[...]}`; wrong-shape payloads; `sleep 5` under a 1 s deadline",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Flags: scope rules, --config replay, --no-input, prompts on /dev/tty, host refusal, exit mapping",
    "check": "cargo test -p toolu-plugins -- args replay prompt && cargo test -p toolu-cli --test plugins_args",
    "ac_refs": ["AC-4", "AC-9", "AC-11"],
    "depends_on": ["S2"],
    "paths": ["crates/plugins/src", "crates/cli/tests/plugins_args.rs", "crates/cli/tests/helpers"],
    "input": "selection files with version 2, generatorVersion 99.0.0, empty enabled, names plus --config; PATH with claude and codex; --host cursor; prompt answers '', '1 3', '2,x', 'q' and EOF over in-memory readers",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "OpenCode: JSONC edits, entries and merge rules, selection, paths, install/list/remove/update",
    "check": "cargo test -p toolu-plugins -- opencode jsonc apply",
    "ac_refs": ["AC-2", "AC-12"],
    "depends_on": ["S2"],
    "paths": ["crates/plugins/src", "crates/plugins/Cargo.toml", "Cargo.lock", "deny.toml"],
    "input": "the COMMENTED, strict, inline, tuple and block-comment fixtures of tools/toolu-cli/src/opencode/__tests__/config.test.ts; R1-R3 merge cases; a symlinked and a mode-600 config; a config rewritten between plan and apply",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "npm wrapper toolu-plugins; delete the TS installer; the Bun OpenCode suite runs the Rust binary",
    "check": "cargo build -p toolu-cli && bun test tools/toolu-cli tooling/src/__tests__/npx-invocation.test.ts tooling/src/__tests__/npm-publish.test.ts",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["S3", "S4"],
    "paths": ["tools/toolu-cli", "tooling/src/__tests__/npx-invocation.test.ts", "tooling/src/__tests__/npm-publish.test.ts", "package.json", "bun.lock", "knip.json", ".jscpd.json", "tsconfig.json", "crates/plugins/src"],
    "input": "isolated HOME/XDG profiles with git projects (the TS cli-fixture); a PATH whose `toolu` is a symlink to the wrapper; an empty temp directory and this repository as cwd",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Smoke, pack and workspace tooling follow the wrapper and the native binary",
    "check": "cargo build -p toolu-cli && bun run smoke:opencode-entry cli.install cli.lifecycle && cargo xtask gate --only pack --only workspace",
    "ac_refs": ["AC-6", "AC-7"],
    "depends_on": ["S5"],
    "paths": ["tooling/src/opencode-host/scenarios-cli.ts", "tooling/src/check-workspace.ts", "tooling/src/pack-inventory.ts", "crates/xtask/src/pack.rs", "crates/xtask/src/package_workspace.rs", "tools/toolu-cli", "crates/plugins/src"],
    "input": "the pinned opencode-ai host and a packed @toolu/opencode tarball; npm pack --dry-run of tools/toolu-cli/npm",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "Agent-facing text names toolu plugins: session notices (TS bundle and Rust), uninstall hint",
    "check": "cargo test -p toolu-hub -p toolu-runtime && bun run build:plugins --check && cargo xtask check-markdown-cli && ! git grep -n 'npx @toolu/plugins' -- plugins/toolu crates/toolu crates/core",
    "ac_refs": ["AC-8"],
    "depends_on": ["S1"],
    "paths": ["plugins/toolu", "crates/toolu/src/lifecycle", "crates/core/runtime/src/host", "docs/cli/commands.json"],
    "input": "the codex delivery-flow install notice; the opencode uninstall command",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "Docs: installer guide, AGENTS.md, README, install/opencode/testing guides, generated CLI reference and copies",
    "check": "cargo build -p toolu-cli && cargo xtask docs-cli && cargo test -p toolu-cli --test contract && cargo xtask gate --only docs-cli --only cli-compat --only context-budget && cargo xtask check-markdown-cli && bun run check:opencode-docs",
    "ac_refs": ["AC-1", "AC-8"],
    "depends_on": ["S5", "S6", "S7"],
    "paths": ["docs", "AGENTS.md", "README.md", "crates/plugins/README.md", "crates/cli/tests/fixtures/commands_json.snap", "tools/toolu-opencode/generated", "tools/toolu-cli/npm/README.md"],
    "input": "the built binary's --help and commands --json",
    "model": "inherit"
  },
  {
    "id": "S9",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": ["AC-10"],
    "depends_on": ["S8"],
    "paths": ["."],
    "input": "the whole workspace",
    "model": "inherit"
  }
]
```

## Critical files

- New: `crates/plugins/{Cargo.toml,README.md}`, `crates/plugins/src/**` and
  `src/**/tests/*_test.rs`; `crates/cli/tests/plugins_hosts.rs`,
  `crates/cli/tests/plugins_args.rs`; `tools/toolu-cli/src/cli.ts` (wrapper)
  and `tools/toolu-cli/src/__tests__/wrapper.test.ts`; the Bun OpenCode suite
  moved to `tools/toolu-cli/src/__tests__/`.
- Changed: `Cargo.toml`, `Cargo.lock`, `crates/cli/{Cargo.toml,src/registry.rs}`,
  `tooling/conventions/guardrails/rust/folders.json`,
  `tools/toolu-cli/{package.json,npm/package.json,npm/README.md,tsconfig.json,guardrails.config.json}`,
  `package.json`, `bun.lock`, `knip.json`, `.jscpd.json`,
  `tooling/src/{opencode-host/scenarios-cli.ts,check-workspace.ts,pack-inventory.ts}`,
  `tooling/src/__tests__/{npx-invocation,npm-publish}.test.ts`,
  `crates/xtask/src/{pack,package_workspace}.rs`,
  `plugins/toolu/hooks/src/lifecycle/session-notices.ts` and its bundle,
  `crates/toolu/src/lifecycle/notices.rs`, `crates/core/runtime/src/host/roots.rs`,
  docs listed in the spec.
- Deleted: the rest of `tools/toolu-cli/src`.

## Verification

End to end: the built binary installs, lists, removes and updates on
OpenCode in isolated profiles (Bun suite), on the pinned OpenCode host
(smoke), and drives stubbed `claude`/`codex` CLIs for core-failure and
ambiguity paths; the wrapper runs it from the repository and from an empty
directory and refuses to run itself. Failure checks: malformed JSONC, both
scopes, bad selection, newer `--config` major, unknown plugin, missing host,
`--host cursor`. Docs: S8 regenerates the CLI reference and checks Markdown
commands. Delivery: scoped commits, `plan-ledger run <plan> --verify`,
`toolu-review:review`, `verdict status` ready, push, PR, babysit.
