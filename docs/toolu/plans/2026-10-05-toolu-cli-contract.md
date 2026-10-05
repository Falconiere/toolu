# The `toolu` CLI contract — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-toolu-cli-contract-design.md   **Topic:** #442 — clap command tree with a namespace per plugin, the output and exit-code contract, `toolu commands --json`, generated `docs/cli/`, and the compatibility, startup and inventory checks.

## Evidence and approach

- **Base.** `origin/main` after PR #476 (#412) merges. #476 adds `crates/cli` (`main.rs`, `args.rs`, `hook.rs`, `output.rs`, `session_start.rs`), `toolu-protocol::{event,install,launcher,stdin}`, `toolu-runtime::{install,invocation,manifest,skew,version}`, the xtask `--bin` option and the `print-hook`/`check-hooks`/`launcher-e2e` tasks. These steps edit those files in place. If #476 is still open when execution starts, S1–S3 and S7–S10 go first, because they do not touch its files, and the rest wait for the rebase.
- **Inspected.**
  - `Cargo.toml` (workspace dependencies are product data, only `[workspace.lints]` is gate data: `gate_change/classify.rs`);
  - `layers.json` (hub `toolu`, rules `ts-quality`/`python-quality`/`rust-quality`/`ast-grep`, binary `cli`);
  - `rules.json` (the stdio owners include `toolu-cli`'s `output` module; env owners are `toolu-runtime` and xtask; only `include!` is banned, not `include_str!`);
  - `folders.json`, `inventory.json`, xtask `gate.rs` (step runner), `gate_change.rs` (merge-base and git helpers), `measure.rs`, `options.rs`;
  - the conformance `percentile` (nearest-rank);
  - `docs/resource-budgets.md` (the startup row);
  - `tooling/src/check-opencode-docs.ts` (`TARGET_DOCS` lists `docs/cli.md`);
  - the OpenCode surface generator, which copies the link closure of skills, so `bun run generate:opencode-surface` must follow the doc move;
  - `.github/ci-paths.json`.
- **Recalled.** Comemory `be52369e` and `b213d153`: on this root host `bun run test` has environmental failures that also fail on `origin/main` (chmod as root, merged `/bin`, `cap_sys_ptrace`). Baseline against a clean `origin/main` worktree before treating a failure as a regression. CI is the authority.
- **Dependencies.** A probe workspace with the repo's `deny.toml` passed `cargo deny check` with clap (builder, no default features), insta, assert_cmd and jsonschema 0.58 (`default-features = false`).
- **Toolchain.** Cargo 1.99 lives in `~/.cargo/bin`; `/usr/bin/cargo` is 1.93. Every check exports `PATH="$HOME/.cargo/bin:$PATH"`. Gate and test runs go through the epic job lease (`job.ts`). Run `bun install --frozen-lockfile` once before S8: the worktree has no `node_modules`, and jscpd and the docs checks need it. Stage new files (`git add`) before a check that reads tracked files (`check:ci-paths`, `check:opencode-surface`, the gate's `check-reach`).

## Workstream summary

Shared types (protocol `Exit`/`Host`, runtime `Ctx`/`Outcome`/`Planned`/`Guide`) → twelve plugin crates → the `crates/cli` clap front end with the fast path → `commands` export and schema → black-box, snapshot and inventory tests → xtask `docs-cli` and the generated `docs/cli/` with the installer guide moved → xtask `check-cli-compat` → xtask `check-startup` and CI → AGENTS.md, template and budget docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1-protocol-types",
    "title": "toolu-protocol: Exit (0,1,2,64,69,75 with names and meanings) and Host (claude, codex, opencode, cursor, hermes) with colocated tests",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-protocol --locked exit:: && cargo test -p toolu-protocol --locked host::",
    "ac_refs": ["AC-5"],
    "paths": ["crates/core/protocol/src/lib.rs", "crates/core/protocol/src/exit.rs", "crates/core/protocol/src/host.rs", "crates/core/protocol/src/tests/exit_test.rs", "crates/core/protocol/src/tests/host_test.rs"],
    "input": "every Exit variant and its code; host names including an unknown 'bogus' and an upper-case 'Codex'",
    "model": "inherit"
  },
  {
    "id": "S2-runtime-cli",
    "title": "toolu-runtime: Ctx, Outcome, and the Planned/Guide namespace descriptors (clap builder) with colocated tests; workspace clap dependency",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-runtime --locked cli:: && cargo test -p toolu-runtime --locked namespace::",
    "ac_refs": ["AC-2", "AC-5"],
    "depends_on": ["S1-protocol-types"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates/core/runtime/Cargo.toml", "crates/core/runtime/src/lib.rs", "crates/core/runtime/src/cli.rs", "crates/core/runtime/src/namespace.rs", "crates/core/runtime/src/tests/cli_test.rs", "crates/core/runtime/src/tests/namespace_test.rs"],
    "input": "the epic Planned descriptor (20 verbs, issues 434/435/448), a Planned with no verbs (doctor), the brainstorm Guide; text and --json contexts; a missing verb",
    "model": "inherit"
  },
  {
    "id": "S3-plugin-crates",
    "title": "Twelve plugin crates (hub toolu-hub with seven toolu namespace modules and the rule re-exports; four rule crates; seven leaf crates) registered in Cargo members and folders.json; each pins PLUGIN to its directory",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test --locked -p toolu-hub -p toolu-ts-quality -p toolu-python-quality -p toolu-rust-quality -p toolu-ast-grep -p toolu-brainstorm -p toolu-delivery-flow -p toolu-review -p toolu-jev -p toolu-statusline -p toolu-pr-babysit -p toolu-epic-orchestrator && cargo xtask check-layers && cargo xtask guardrails",
    "ac_refs": ["AC-1", "AC-2", "AC-3"],
    "depends_on": ["S2-runtime-cli"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates/toolu", "crates/ts-quality", "crates/python-quality", "crates/rust-quality", "crates/ast-grep", "crates/brainstorm", "crates/delivery-flow", "crates/toolu-review", "crates/jev", "crates/statusline", "crates/pr-babysit", "crates/epic-orchestrator", "tooling/conventions/guardrails/rust/folders.json", "tooling/conventions/guardrails/rust/layers.json"],
    "input": "each crate's real command() and run() for its planned verb, text and --json; the hub's rule re-exports; a cli→rule edge is not introduced",
    "model": "inherit"
  },
  {
    "id": "S4-cli-front-end",
    "title": "crates/cli: fast path (hook forms, --hook-protocol) before clap; clap root with global --json/--quiet/--host/--config-dir, --version and hidden --hook-protocol; registry with owners, hidden hook verbs and plugin aliases; dispatch to namespaces and #412's hook runner; clap-error mapping (64, help 0, JSON envelope); output honours --quiet",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-cli --locked --bin toolu && cargo test -p toolu-cli --locked --bin toolu never_builds_the_tree 2>&1 | grep -q 'test result: ok. [1-9]'",
    "ac_refs": ["AC-5", "AC-6", "AC-7", "AC-10"],
    "depends_on": ["S3-plugin-crates"],
    "paths": ["crates/cli/Cargo.toml", "crates/cli/src/main.rs", "crates/cli/src/fast.rs", "crates/cli/src/tree.rs", "crates/cli/src/registry.rs", "crates/cli/src/dispatch.rs", "crates/cli/src/clap_error.rs", "crates/cli/src/output.rs", "crates/cli/src/hook.rs", "crates/cli/src/tests"],
    "input": "main_test runs run(words, &context, &recording_tree) where the tree builder counts its calls: hook pre-tools --event PreToolUse --plugin-root DIR and jev hook session-start --event SessionStart and --hook-protocol → 0 builds (test never_builds_the_tree); --version → 1 build and no namespace run is called; malformed hook pre-tools --bogus x → 1 build and exit 64. Also: epik start (64 + epic tip); --json epik start (error document); epic (help on stderr, 64); --host bogus commands (64); a success Outcome with stderr under --quiet (dropped) and a failing one (kept)",
    "model": "inherit"
  },
  {
    "id": "S5-commands-export",
    "title": "toolu commands: human listing, --json tree (toolu.commands/v1, no version, hookProtocol, exit codes, owners, hidden, placeholder) and --schema (draft 2020-12 with $defs for every --json document)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p toolu-cli --locked --bin toolu",
    "ac_refs": ["AC-4"],
    "depends_on": ["S4-cli-front-end"],
    "paths": ["crates/cli/src/commands.rs", "crates/cli/src/export.rs", "crates/cli/src/commands.schema.json", "crates/cli/src/tests/commands_test.rs", "crates/cli/src/tests/export_test.rs"],
    "input": "the real full tree built by tree::command(); a hidden verb, an alias, a global flag and a flag with possible values",
    "model": "inherit"
  },
  {
    "id": "S6-black-box-tests",
    "title": "crates/cli integration tests: contract.rs (assert_cmd: every namespace --help, exit codes and both streams, scenarios), commands.rs (jsonschema validation of every --json document + insta snapshot in tests/fixtures), inventory.rs (12 manifests = 12 crates = 12 owners, and each side's removal is named)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; CI=1 cargo test -p toolu-cli --locked --test contract",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-7"],
    "depends_on": ["S5-commands-export"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates/cli/Cargo.toml", "crates/cli/src", "crates/cli/tests/contract.rs", "crates/cli/tests/helpers", "crates/cli/tests/fixtures", "plugins", "crates", "tooling/conventions/guardrails/rust/layers.json"],
    "input": "the built toolu binary; toolu epic status 402 --json; toolu epik start; toolu hook pre-tools --event PreToolUse; every visible namespace --help; a JSON document with a required key dropped (fails the schema); the repository's plugins/*/.claude-plugin, crates/* and the tree owners, then each of the three sets with one name removed → the inventory names that name and the side that lacks it",
    "model": "inherit"
  },
  {
    "id": "S7-docs-cli",
    "title": "cargo xtask docs-cli [--check] [--bin]: render docs/cli/README.md, one page per visible top-level command from real --help output, commands.json and commands.schema.json with a generated marker; --check reports stale, missing and orphaned marker files; gate step docs-cli; inventory entries",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked --bin xtask docs_cli && cargo xtask docs-cli --check",
    "ac_refs": ["AC-8"],
    "depends_on": ["S6-black-box-tests"],
    "paths": ["crates/xtask/src/main.rs", "crates/xtask/src/options.rs", "crates/xtask/src/gate.rs", "crates/xtask/src/docs_cli.rs", "crates/xtask/src/docs_cli", "crates/xtask/src/tests", "tooling/conventions/guardrails/rust/inventory.json", "crates/cli/src", "crates/cli/Cargo.toml", "crates", "docs/cli"],
    "input": "the real toolu built by cargo against the committed docs/cli; a stand-in toolu with an empty temp --root (all missing), a written root (clean), an edited page (stale), an orphaned marker page, an unmarked installer.md (untouched)",
    "model": "inherit"
  },
  {
    "id": "S8-generate-docs-and-fold",
    "title": "Generate docs/cli/ with cargo xtask docs-cli; git mv docs/cli.md docs/cli/installer.md; update references (AGENTS.md, README.md, docs/opencode.md, docs/plugins/index.md, tools/toolu-cli/npm/README.md, check-opencode-docs.ts and its test); regenerate the OpenCode surface",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask docs-cli --check && test ! -e docs/cli.md && bun run check:opencode-docs && bun run check:opencode-surface && bun test --timeout 60000 tooling/src/__tests__/check-opencode-docs.test.ts && ! git grep -n 'docs/cli\\.md\\|(cli\\.md\\|\\.\\./cli\\.md' -- ':!CHANGELOG.md' ':!docs/toolu' ':!docs/releases'",
    "ac_refs": ["AC-8", "AC-12"],
    "depends_on": ["S7-docs-cli"],
    "paths": ["docs/cli", "docs/cli.md", "AGENTS.md", "README.md", "docs/opencode.md", "docs/plugins/index.md", "tools/toolu-cli/npm/README.md", "tooling/src/check-opencode-docs.ts", "tooling/src/__tests__/check-opencode-docs.test.ts", "tools/toolu-opencode/generated", "crates/cli/src"],
    "input": "the real binary's tree; the moved installer guide",
    "model": "inherit"
  },
  {
    "id": "S9-cli-compat",
    "title": "cargo xtask check-cli-compat [--base] [--title]: compare docs/cli/commands.json at the merge base with the working tree (names, aliases, flags, shorts, possible values, exit codes, optional→required; placeholders exempt; hookProtocol bump + ! title allows); gate step cli-compat; inventory entries",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked --bin xtask cli_compat && cargo xtask check-cli-compat",
    "ac_refs": ["AC-9"],
    "depends_on": ["S8-generate-docs-and-fold"],
    "paths": ["crates/xtask/src/main.rs", "crates/xtask/src/gate.rs", "crates/xtask/src/cli_compat.rs", "crates/xtask/src/cli_compat", "crates/xtask/src/tests", "tooling/conventions/guardrails/rust/inventory.json", "docs/cli/commands.json"],
    "input": "the committed docs/cli/commands.json and copies with a removed verb, alias, flag, short, possible value, exit code, an optional→required arg, a removed placeholder, and a hookProtocol bump with and without a feat! title; a base without the file",
    "model": "inherit"
  },
  {
    "id": "S10-startup",
    "title": "cargo xtask check-startup --bin: 3 warm-up + 30 timed spawns of <bin> --version, nearest-rank p50 against benchmarks/startup-budgets.json; Linux rust CI step on the release binary; ci-paths rust group gains docs/cli/** and the budget file; inventory entries",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked --bin xtask startup && cargo build --release --locked -p toolu-cli && cargo xtask check-startup --bin target/release/toolu && grep -q 'xtask check-startup' .github/workflows/tests.yml && bun run check:ci-paths",
    "ac_refs": ["AC-10"],
    "depends_on": ["S4-cli-front-end", "S8-generate-docs-and-fold"],
    "paths": ["crates/xtask/src/main.rs", "crates/xtask/src/startup.rs", "crates/xtask/src/tests", "benchmarks/startup-budgets.json", ".github/workflows/tests.yml", ".github/ci-paths.json", "tooling/conventions/guardrails/rust/inventory.json", "crates/cli/src"],
    "input": "the release toolu built by cargo (check-startup --bin target/release/toolu); a stand-in printing toolu 0.0.0 (a_fast_binary_passes); a script that sleeps 10 ms before printing toolu 0.0.0 (a_slow_binary_fails_naming_the_budget → exit 1); a script printing the wrong version text (exit 2); a malformed budget file (exit 2); the tests.yml step on ubuntu-latest",
    "model": "inherit"
  },
  {
    "id": "S11-docs",
    "title": "AGENTS.md (CLI bullet, plugin = crate + Markdown, key files, xtask commands, rust CI paths, contributing steps for a namespace change), tooling/templates/plugin-README.md (crate and namespace section), docs/resource-budgets.md (startup measurement and gate), installer.md pointer to README.md",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; grep -q 'cargo xtask docs-cli' AGENTS.md && grep -q 'check-cli-compat' AGENTS.md && grep -q 'check-startup' docs/resource-budgets.md && grep -q 'toolu <namespace>' tooling/templates/plugin-README.md && bun run guardrails && bun run check:retired-plugins && bun run check:ci-paths",
    "ac_refs": ["AC-12"],
    "depends_on": ["S8-generate-docs-and-fold", "S9-cli-compat", "S10-startup"],
    "paths": ["AGENTS.md", "tooling/templates/plugin-README.md", "docs/resource-budgets.md", "docs/cli/installer.md", ".github/ci-paths.json", "tooling/src/check-retired-plugin-references.ts", "tooling/src/__tests__/check-retired-plugin-references.test.ts", "crates"],
    "input": "the final command surface and xtask tasks",
    "model": "inherit"
  },
  {
    "id": "S12-full-gate",
    "title": "Full Rust gate and TypeScript suite on the final tree (environmental failures baselined against a clean origin/main worktree)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(cli): the toolu CLI contract, one binary with a namespace per plugin (#442)'",
    "ac_refs": ["AC-11"],
    "depends_on": ["S11-docs"],
    "paths": ["Cargo.toml", "Cargo.lock", "crates", "docs/cli", "tooling/conventions/guardrails/rust", "benchmarks/startup-budgets.json"],
    "input": "the whole workspace",
    "model": "inherit"
  }
]
```

## Critical files

- **Protocol and runtime:**
  - `crates/core/protocol/src/{lib,exit,host}.rs`, `crates/core/protocol/src/tests/{exit,host}_test.rs`
  - `crates/core/runtime/{Cargo.toml,src/lib.rs,src/cli.rs,src/namespace.rs}`, `crates/core/runtime/src/tests/{cli,namespace}_test.rs`
- **Plugin crates:**
  - `crates/{toolu,ts-quality,python-quality,rust-quality,ast-grep,brainstorm,delivery-flow,toolu-review,jev,statusline,pr-babysit,epic-orchestrator}/{Cargo.toml,src/lib.rs,src/tests/lib_test.rs}`
  - the hub's modules: `crates/toolu/src/{ledger,debug,setup,doctor,config,status,serve}.rs` with tests.
- **CLI:**
  - `crates/cli/Cargo.toml`
  - `crates/cli/src/{main,fast,tree,registry,dispatch,clap_error,commands,export,output,hook}.rs`, `crates/cli/src/commands.schema.json`
  - `crates/cli/src/tests/*_test.rs` (`args_test.rs` becomes `fast_test.rs`)
  - `crates/cli/tests/{contract,commands,inventory}.rs`, `crates/cli/tests/helpers/*.rs`, `crates/cli/tests/fixtures/commands__commands_json.snap`
- **xtask:**
  - `crates/xtask/src/{main,options,gate,docs_cli,cli_compat,startup}.rs` (submodules if a file nears 300 code lines), with unit tests
- **Data:** `Cargo.toml`, `Cargo.lock`, `tooling/conventions/guardrails/rust/{folders,inventory}.json`, `benchmarks/startup-budgets.json`.
- **CI:** `.github/ci-paths.json`, `.github/workflows/tests.yml`.
- **Docs:**
  - `docs/cli/{README.md,<command>.md,commands.json,commands.schema.json,installer.md}` (`docs/cli.md` removed)
  - `AGENTS.md`, `README.md`, `docs/opencode.md`, `docs/plugins/index.md`, `docs/resource-budgets.md`
  - `tools/toolu-cli/npm/README.md`, `tooling/templates/plugin-README.md`
  - `tooling/src/check-opencode-docs.ts` (and its test, if it names the path)
  - `tools/toolu-opencode/generated/**` (regenerated)

## Verification

- **End to end.**
  - The release `toolu` answers `--help`, `--version` and every namespace's `--help`, and `commands --json` validates against `--schema`.
  - `toolu epic status 402 --json` prints exactly one JSON document (exit 64 until #434).
  - `toolu epik start` suggests `epic` (exit 64), and `toolu hook pre-tools --event PreToolUse` exits 2.
  - S4's unit test proves the fast path builds no tree.
- **Failure and boundary.**
  - Malformed hook line → 64 through clap.
  - `--host bogus` → 64.
  - Missing verb → help on stderr, 64.
  - `docs-cli --check` on stale, missing and orphaned files → 1.
  - `check-cli-compat` on each breaking edit → 1, and on placeholder removal or a bump with `!` → 0.
  - `check-startup` on a slow binary → 1, on wrong output → 2.
  - Inventory with each side short by one → names it.
- **Docs sync.** S8 and S11 are runnable checks, and `cargo xtask docs-cli --check` runs in the gate.
- **Delivery.**
  - After S12: scoped commits, then `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run docs/toolu/plans/2026-10-05-toolu-cli-contract.md --verify`.
  - Then `toolu-review:review` with version 2 state covering every changed file, and `verdict.js status` reporting `overall: ready`.
  - Then push, a PR against `main` whose body starts with `Closes Falconiere/toolu#442` and `Part of Falconiere/toolu#402`, and `/pr-babysit:babysit`.
  - Prerequisites: `gh api user` succeeds, the branch is `feat/442-the-toolu-cli-contract-one`, and the pr-babysit skill is installed.
- **Full suite.** `bun run test` runs before delivery. Failures are compared with the same command on a clean `origin/main` worktree (comemory `be52369e`, `b213d153`), and only identical environmental failures are accepted, recorded in the PR body.

## Plan review

Round 1 (Needs changes), fixed in place:

- S10: 🔴 blocker: `check:ci-paths` rejects a glob that matches no tracked file, and `docs/cli/**` exists only after S8. S10 now depends on S8, and the evidence note says to stage new files before tracked-file checks.
- S5: 🟡 should-fix: the `commands` test filter skipped `export` tests. The check now runs every unit test of the binary.
- S8: 🟡 should-fix: the docs checks need `node_modules`, which this worktree lacks. `bun install --frozen-lockfile` comes first, and a `git grep` proves no reference to `docs/cli.md` remains.
- S4: 🟡 should-fix: the input did not name the recording tree-builder test. The input and check now name `never_builds_the_tree`, and S4 also covers AC-10's "no namespace work at startup".
- S6 and S10: 🔵 consider: the inventory input names each side's removal; the startup input names the slow-binary and wrong-output cases and the CI step, which the check greps.
- S7, S9, S10: 🔵 consider: unit filters are scoped to `--bin xtask`.

Jev (`jev-1.13.0`, `--raw`), step-versus-AC alignment:

| Pair | Round 1 | Revised |
|---|---|---|
| S4 ↔ AC-6 | 0.17 | 0.58 |
| S10 ↔ AC-10 | 0.60 | 0.63 |
| S8 + S11 ↔ AC-12 | 0.69 | 0.71 |
| S6 ↔ AC-3 | 0.61 | 0.75 |
| a happy-path-only step exists | 0.32 | 0.29 |

The named-test checks were then added for the S4 and S10 cases Jev could not see in the commands. `checkAcRefs`: ok, no dangling refs; all 12 ACs are covered.

Round 2: Approved.

## Deviations

- **S6, one test binary.** The three integration files became one binary, `crates/cli/tests/contract.rs`, with `helpers/{cli,schema,inventory}.rs` as modules. That is #412's `launcher.rs` pattern. A shared helper included in three binaries trips the deny-warnings `dead_code` lint wherever one helper goes unused, and one binary compiles once. The snapshot is `crates/cli/tests/fixtures/commands_json.snap` (no module prefix). The spec's AC-4 evidence row names the old file and test names; the check and assertions are unchanged.
- **Job admission.** From 22:58 the worktree's agent lease has stood at stage `uncertain` (owner pid gone), so `job.ts` and the ledger refuse every job (`worktree job admission blocked by agent stage uncertain`). Reported to the orchestrator. Until it clears, targeted `cargo test -p <crate>` runs go direct at low load; the ledger stamps and the full gate wait for admission.
- **S7, S9, S10, no xtask integration targets.** The checks named `cargo test -p xtask --test docs_cli|cli_compat|startup`, which were never written. The real `toolu` cannot be a dependency of xtask (the layer rule), so those targets would nest `cargo build -p toolu-cli` inside `cargo test`, also under the gate's `cargo llvm-cov`. #412's `launcher-e2e` set the pattern instead: pass and fail unit tests in `crates/xtask/src/tests/` with a stand-in binary, and a real-binary run as the check. So S7 runs `cargo xtask docs-cli --check` (cargo builds the real `toolu`), S9 runs `cargo xtask check-cli-compat` on the real tree, and S10 runs `cargo xtask check-startup --bin target/release/toolu`. Jev agreed (`choice`, amend 0.94).
- **S11, the epic crate's `jira` verb.** `bun run check:retired-plugins` flagged `crates/epic-orchestrator/src/lib.rs` and its test. The word there is the planned `toolu epic jira` verb, the built-in tracker that #404 keeps, not the retired standalone plugin. The check already exempts `plugins/epic-orchestrator/`. A plugin is now its crate plus Markdown, so it exempts `crates/epic-orchestrator/` too. Its test gained the crate case: red before the change, green after.
