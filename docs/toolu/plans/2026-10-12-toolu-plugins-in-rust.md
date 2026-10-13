# `toolu plugins`: the installer in Rust — Plan

**Date:** 2026-10-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-12-toolu-plugins-in-rust-design.md   **Topic:** #438: port `tools/toolu-cli` to `crates/plugins` as `toolu plugins`, behind a thin `@toolu/plugins` wrapper

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
    "check": "cargo test -p toolu-plugins -- catalog:: order:: && cargo build -p toolu-cli && ./target/debug/toolu plugins --help | grep -q install && ! ./target/debug/toolu plugins --help | grep -q planned && cargo xtask gate --only deny --only guardrails --only layers",
    "ac_refs": [
      "AC-1"
    ],
    "paths": [
      "Cargo.toml",
      "Cargo.lock",
      "crates/plugins",
      "crates/cli/Cargo.toml",
      "crates/cli/src/registry.rs",
      "crates/cli/src/tests/registry_test.rs",
      "tooling/conventions/guardrails/rust/folders.json",
      ".claude-plugin/marketplace.json"
    ],
    "input": "the real .claude-plugin/marketplace.json; a catalog with a cycle; an unknown name",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Claude Code and Codex: adapters, detection, install/list/remove/update, step runner with deadline, text and JSON reports",
    "check": "cargo test -p toolu-plugins -- host:: verbs:: report:: && cargo test -p toolu-cli --test plugins_hosts",
    "ac_refs": [
      "AC-3",
      "AC-9",
      "AC-12"
    ],
    "depends_on": [
      "S1"
    ],
    "paths": [
      "crates/plugins/src",
      "crates/cli/tests/plugins_hosts.rs",
      "crates/cli/tests/helpers"
    ],
    "input": "a temp PATH whose claude script fails only `plugin install toolu@toolu`, logs argv and prints recorded `plugin list --json` payloads; a codex payload `{installed:[...]}`; wrong-shape payloads; `sleep 5` under a 1 s deadline",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Flags: scope rules, --config replay, --no-input, prompts on /dev/tty, host refusal, exit mapping",
    "check": "cargo test -p toolu-plugins -- args:: replay:: prompt:: && cargo test -p toolu-cli --test plugins_args",
    "ac_refs": [
      "AC-4",
      "AC-9",
      "AC-11"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/plugins/src",
      "crates/cli/tests/plugins_args.rs",
      "crates/cli/tests/helpers"
    ],
    "input": "selection files with version 2, generatorVersion 99.0.0, empty enabled, names plus --config; PATH with claude and codex; --host cursor; prompt answers '', '1 3', '2,x', 'q' and EOF over in-memory readers",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "OpenCode: JSONC edits, entries and merge rules, selection, paths, install/list/remove/update, concurrent-edit check",
    "check": "cargo test -p toolu-plugins -- opencode:: && cargo xtask gate --only guardrails --only layers --only unused-pub --only reach",
    "ac_refs": [
      "AC-2",
      "AC-12"
    ],
    "depends_on": [
      "S2"
    ],
    "paths": [
      "crates/plugins/src",
      "crates/plugins/Cargo.toml",
      "Cargo.lock"
    ],
    "input": "the COMMENTED, strict, inline, tuple and block-comment fixtures of tools/toolu-cli/src/opencode/__tests__/config.test.ts (tests jsonc_test::append_*, remove_*, block_comment_*); R1-R3 merge cases; a symlinked and a mode-600 config; a config rewritten between plan and apply",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "npm wrapper toolu-plugins; delete the TS installer; the Bun OpenCode suite and resolution helper run the Rust binary",
    "check": "cargo build -p toolu-cli && bun test --timeout 60000 tools/toolu-cli tooling/src/__tests__/npx-invocation.test.ts tooling/src/__tests__/npm-publish.test.ts tooling/src/__tests__/workspace-skeleton.test.ts && cargo test -p toolu-cli --test doctor --test launcher -- npm_wrapper",
    "ac_refs": [
      "AC-5",
      "AC-7"
    ],
    "depends_on": [
      "S3",
      "S4"
    ],
    "paths": [
      "tools/toolu-cli",
      "tooling/src/__tests__/npx-invocation.test.ts",
      "tooling/src/__tests__/npm-publish.test.ts",
      "tooling/src/__tests__/workspace-skeleton.test.ts",
      "tools/toolu-conformance/src/harness/entry-command.ts",
      "tooling/src/check-retired-plugin-references.ts",
      "crates/cli/tests/helpers/resolution.rs",
      "crates/cli/tests/doctor.rs",
      ".github/workflows/npm-publish.yml",
      "package.json",
      "bun.lock",
      "knip.json",
      ".jscpd.json",
      "tsconfig.json",
      "crates/plugins/src",
      "crates/cli/tests/launcher.rs"
    ],
    "input": "isolated HOME/XDG profiles with git projects (the cli-fixture, now spawning requiredBuiltTooluBinary(), which throws when no binary is built); wrapper.test.ts cases: a PATH whose `toolu` is a symlink to the wrapper (69), a native stub that answers --hook-protocol but not plugins (69), TOOLU_BIN set (runs, from this repository and an empty dir), --version with no native binary, a child killed by SIGTERM (re-raised); resolution.rs and doctor.rs keep their rejection coverage against a legacy `toolu` Node script (the old global @toolu/plugins bin, written by the test) instead of building the deleted CLI",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Smoke, doc-command shims, pack, workspace and CI path tooling follow the wrapper and the native binary",
    "check": "cargo build -p toolu-cli && bun test --timeout 60000 tooling/src/opencode-host/__tests__ tooling/src/__tests__/pack-inventory.test.ts && bun run smoke:opencode-entry cli.install cli.lifecycle && cargo xtask gate --only pack --only workspace --only ci-paths",
    "ac_refs": [
      "AC-6",
      "AC-7"
    ],
    "depends_on": [
      "S5"
    ],
    "paths": [
      "tooling/src/opencode-host",
      "tooling/src/check-workspace.ts",
      "tooling/src/pack-inventory.ts",
      "tooling/src/__tests__/pack-inventory.test.ts",
      "crates/xtask/src/pack.rs",
      "crates/xtask/src/package_workspace.rs",
      ".github/ci-paths.json",
      "tools/toolu-cli",
      "crates/plugins/src"
    ],
    "input": "the pinned opencode-ai host and a packed @toolu/opencode tarball; npm pack --dry-run of tools/toolu-cli/npm; the docs shim for `npx @toolu/plugins`",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "Agent-facing text names toolu plugins: session notices (TS and Rust, golden), uninstall hint (TS and Rust), plugin skills",
    "check": "cargo build -p toolu-cli && cargo xtask docs-cli && cargo test -p toolu-hub -p toolu-runtime && bun test --timeout 60000 plugins/toolu/hooks/src/__tests__/lifecycle-golden.test.ts packages/toolu-core/src/host tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts && bun run build:plugins --check && cargo xtask check-markdown-cli && ! git grep -n 'npx @toolu/[p]lugins' -- plugins ':!plugins/*/README.md' crates/toolu crates/core crates/cli/src packages/toolu-core/src fixtures",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "S3"
    ],
    "paths": [
      "plugins",
      "crates/toolu/src/lifecycle",
      "crates/core/runtime/src/host",
      "packages/toolu-core/src/host",
      "fixtures/gates/lifecycle-golden.json",
      "docs/cli/commands.json",
      "docs/cli",
      "crates/plugins/src",
      "tools/toolu-opencode/scripts"
    ],
    "input": "the codex delivery-flow install notice; the opencode uninstall command; the epic-orchestrator skill's install line",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "Docs: installer guide, AGENTS.md, README, guides, generated CLI reference, render text, markdown-cli allow entries, generated copies",
    "check": "cargo build -p toolu-cli && cargo xtask docs-cli && cargo test -p xtask docs_cli && INSTA_UPDATE=always cargo test -p toolu-cli --test contract && cargo test -p toolu-cli --test contract && cargo xtask gate --only docs-cli --only cli-compat --only context-budget && cargo xtask check-markdown-cli && bun run check:opencode-docs && bun test --timeout 60000 tooling/src/__tests__/install-prompts.test.ts tooling/src/opencode-host/__tests__ && ! git grep -n -e 'bun tools/toolu-cli/src/cli.ts' -e 'bun run tools/toolu-cli/src/cli.ts' -- docs README.md AGENTS.md ':!docs/toolu'",
    "ac_refs": [
      "AC-1",
      "AC-8"
    ],
    "depends_on": [
      "S5",
      "S6",
      "S7"
    ],
    "paths": [
      "docs/cli",
      "docs/install.md",
      "docs/config.md",
      "docs/opencode.md",
      "docs/opencode-migration.md",
      "docs/testing.md",
      "docs/plugins/index.md",
      "docs/epic-orchestrator/README.md",
      "AGENTS.md",
      "README.md",
      "crates/plugins/README.md",
      "crates/cli/tests/fixtures/commands_json.snap",
      "crates/xtask/src/docs_cli",
      "tooling/conventions/markdown-cli.json",
      "tooling/src/__tests__/install-prompts.test.ts",
      "tools/toolu-opencode/generated",
      "tools/toolu-cli/npm/README.md",
      "plugins/brainstorm/README.md",
      "plugins/delivery-flow/README.md",
      "plugins/epic-orchestrator/README.md",
      "crates/cli/src/commands.schema.json",
      "crates/cli/tests/contract.rs",
      "crates/cli/tests/helpers",
      "docs/markdown-cli.md",
      "tooling/src/__tests__/install-prompts.test.ts",
      ".github/ci-paths.json"
    ],
    "input": "the built binary's --help and commands --json; the README and docs/plugins/index.md install fences",
    "model": "inherit"
  },
  {
    "id": "S9",
    "title": "Full quality gate",
    "check": "git fetch origin main && cargo xtask gate --base origin/main --title 'feat(plugins)!: toolu plugins, the installer in Rust (#438)'",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "S8"
    ],
    "paths": [
      "."
    ],
    "input": "the whole workspace",
    "model": "inherit"
  }
]
```

## Decisions from plan review

- `deny.toml` is not edited (gate data): `jsonc-parser` is MIT and adds no new
  transitive crate; S1 proves it with `--only deny`.
- `npx @toolu/plugins` stays valid in human-facing docs (`README.md`,
  `docs/**`, plugin READMEs): the wrapper still serves it once `toolu` is
  installed. Agent-facing text (skills, commands, agents, hook notices) and
  the Rust/TS uninstall hints switch to `toolu plugins …` (S7 greps exactly
  those paths). `tools/toolu-cli/src/cli.ts` as a runnable source disappears
  from every doc (S8 grep).
- `HOOK_PROTOCOL` is not bumped: `cli-compat` judges the Rust tree, where only
  the exempt `planned` verb goes. The `!` is for the npm bin rename.
- `cargo xtask gate` locally takes `--base origin/main --title <PR title>`.
- The `commands --json` snapshot is accepted by me in S8
  (`INSTA_UPDATE=always cargo test -p toolu-cli --test contract`, then a clean
  rerun) and committed with that step.
- `tools/toolu-cli/package.json` keeps its `build` script (copy the catalog,
  `bun build src/cli.ts --target=node`) with no dependencies; `knip.json` and
  `.jscpd.json` drop the deleted entries (S5).

## Delivery

After S9: scoped commit(s) `feat(plugins)!: …`; `plan-ledger run <plan> --verify`;
`toolu-review:review` (v2 state over every changed file); `verdict status`
`overall: ready`; push; PR with `Closes Falconiere/toolu#438` / `Part of
Falconiere/toolu#402`; `report pr-open`, `report babysit`, `pr-babysit:babysit`.

## Deviations

- S5 runs only the `npm_wrapper` tests of `doctor.rs` and `launcher.rs`, the
  two that build the wrapper. `doctor_does_not_wait_for_a_descendant_holding_protocol_output_open`
  fails locally under `cargo test` whenever the target dir is on `/Volumes`
  (about 6.5 s, bound 2 s). A clean `origin/main` fails the same way there and
  passes with the target on `/private/tmp`; CI is green. It is unrelated to
  this change and is left alone. S9 records how the full gate copes with it.
- `remove` and `update` on Claude Code and Codex now honour `--dry-run`. The
  TypeScript CLI ignored it and ran the real host commands, which contradicted
  its own help text.
- The wrapper's native-binary probes wait up to 20 s (the spec said 5 s). The
  npx test exceeded 5 s under the ledger's parallel load.

## Critical files

- New: `crates/plugins/{Cargo.toml,README.md}`, `crates/plugins/src/**` and
  `src/**/tests/*_test.rs`; `crates/cli/tests/plugins_hosts.rs`,
  `crates/cli/tests/plugins_args.rs`; `tools/toolu-cli/src/cli.ts` (wrapper)
  and `tools/toolu-cli/src/__tests__/wrapper.test.ts`; the Bun OpenCode suite
  moved to `tools/toolu-cli/src/__tests__/`.
- Also changed (plan review): `crates/cli/tests/helpers/resolution.rs`,
  `tooling/src/opencode-host/{scenarios-cli,doc-commands}.ts` and their tests,
  `fixtures/gates/lifecycle-golden.json`, `packages/toolu-core/src/host/host-roots.ts`,
  `crates/xtask/src/docs_cli/render.rs` and its test,
  `tooling/conventions/markdown-cli.json`, `.github/ci-paths.json`
  (`crates/plugins/**`, `tools/toolu-cli/**` into `opencode`),
  `tooling/src/check-retired-plugin-references.ts`,
  `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`.
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
