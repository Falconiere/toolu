# The `toolu` CLI contract — Design

**Date:** 2026-10-05   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** #442 — one `toolu` binary, a clap namespace per plugin, the output and exit-code contract, `toolu commands --json`, generated `docs/cli/`, and the checks that keep them honest.

Brainstorm: `docs/toolu/brainstorms/2026-10-05-toolu-cli-contract.md`. Builds on #412 (PR #476), which adds `crates/cli` with a hand-written argv parser and the native hook runner.

## Problem

Epic #402 turns every plugin into "a Rust crate plus Markdown". Skills, commands, agents and `hooks.json` will name `toolu …` commands, so the binary's command tree, its streams and its exit codes are a public contract before any plugin is ported. Today `toolu` (#412) understands only `--version`, `--hook-protocol` and the hook forms. There is no namespace per plugin, no machine-readable tree for #444's drift gate or for docs, and nothing stops a later port from silently breaking a verb a plugin already calls. Binaries and plugins are upgraded separately (#411), so such a break would surface as a failing hook in a user's session.

## Non-Goals

1. No namespace is ported. Real verbs, host detection (#414), config loading, `doctor`/`status` (#445) and `serve` (#437) stay with their issues. Unported namespaces get exactly one placeholder verb.
2. #412's hook runner (`crates/cli/src/hook.rs`, `session_start.rs`), its launcher grammar, the skew prelude and the generated `hooks.json` strings do not change. Only the argv front end in front of them changes.
3. No `hooks.json` entry or Markdown file switches to `toolu …` (#425, #443, #444).
4. No `crates/plugins`: `toolu plugins` is a `crates/cli` built-in placeholder until #438 ports the installer into its own crate.
5. No behaviour-inventory kind for CLI verbs: adding a discovery kind changes `rules.json`, which is gate data and must ship in its own `chore(gates):` PR. This PR adds inventory entries only for its new `cargo xtask` tasks.
6. No change to gate data (`[workspace.lints]`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `lang.rust`, `rules.json`, `jscpd.json`). Registration data (`folders.json`, `inventory.json`) grows.
7. No colour, shell completion or man pages.

## Architecture

**One tree, built only when needed.** `crates/cli/src/main.rs` reads argv (`toolu_runtime::invocation::args`) and first tries the fast path (`fast.rs`): exactly `--hook-protocol`, `hook <name> [--event E] [--plugin-root D]`, or `<plugin> hook <name> [...]` with #412's name rule. A fast match runs #412's code and never builds the clap tree. Anything else, including a malformed hook line, goes through clap. `run` takes the tree builder as a parameter (`&dyn Fn() -> clap::Command`), so a unit test can assert the builder is never called on the fast path.

**Namespaces.** `crates/cli/src/registry.rs` lists every top-level command and its owner. The toolu plugin owns `hook` (live, #412's runner) and placeholders for `ledger`, `debug`, `setup`, `doctor`, `config`, `status` and `serve`, which live in the hub crate `crates/toolu`. The hub also re-exports the four rule crates (`ts-quality`, `python-quality`, `rust-quality`, `ast-grep`), because `crates/cli` may not depend on a rule crate (#460, `check-layers`). The leaf crates are `brainstorm` and `delivery-flow` (guide namespaces with no verbs) and `toolu-review` (`review`), `jev`, `statusline`, `pr-babysit` (`babysit`) and `epic-orchestrator` (`epic`). `crates/cli` adds two built-ins: `commands` (live) and `plugins` (placeholder). For every plugin namespace except the toolu plugin's, the registry also adds a hidden `hook` verb, and an alias equal to the plugin directory name where the two differ (`review`/`toolu-review`, `babysit`/`pr-babysit`, `epic`/`epic-orchestrator`). The clap tree therefore accepts and documents every form #412's generator writes.

**Plugin crate API.** Each plugin crate exports `PLUGIN: &str` (its directory name), `command() -> clap::Command` and `run(&ArgMatches, &Ctx) -> Outcome`. The hub has the same three items per toolu namespace module. The issue's `ExitCode` return becomes `Outcome`, for two reasons. Plugin crates may not touch the standard streams (rule 14: only `toolu-protocol`, `toolu-cli`'s `output` module and xtask may), so `crates/cli` prints what `run` returns. And a typed `Exit` keeps the exit-code table in one place. Placeholder and guide crates delegate to shared descriptors in `toolu-runtime` (`Planned`, `Guide`). That keeps 13 crates free of 10-line clones (jscpd) and makes the placeholder text uniform.

**Output.** `output::emit` prints `Outcome.stdout` on stdout and `Outcome.stderr` on stderr. Under `--quiet`, stderr is dropped when the exit is 0. Under `--json`, every non-hook exit path leaves exactly one JSON document on stdout. That includes help, version and usage errors. A verb that fails without a document gets the error envelope; a success without one gets `{}`.

**Machine-readable tree.** `toolu commands --json` walks the built clap tree into `toolu.commands/v1` JSON with no version field, so release bumps never make it stale. `toolu commands --schema` prints the JSON Schema (draft 2020-12) for that document and for every other `--json` document. The schema source is `crates/cli/src/commands/schema.json`, embedded with `include_str!`.

**Generated docs.** `cargo xtask docs-cli` builds `toolu-cli` (or takes `--bin`). It then writes `docs/cli/README.md`, one page per visible top-level command, `commands.json` and `commands.schema.json`. Each page is made of the real `toolu <path> --help` output of every visible command beneath it. Every generated Markdown file starts with a marker line. `--check` regenerates in memory, compares, and reports stale, missing and orphaned marker files. It is a new gate step, `docs-cli`. Unmarked files are hand-written and left alone. That applies to exactly one: `docs/cli.md` moves to `docs/cli/installer.md`, the guide for the Node installer until #438.

**Compatibility.** The new xtask task `check-cli-compat` and gate step `cli-compat` read `docs/cli/commands.json` at `git merge-base <base> HEAD` and in the working tree. The `docs-cli` step before it has already proved the working tree's file matches the binary. A documented name, alias, flag, short flag, possible value or exit code that disappears is breaking, and so is an optional argument that becomes required or a new required argument. Placeholders (`planned`) are exempt. A breaking change is allowed only when the tree's `hookProtocol` increased (#411: bumped when a documented verb breaks). With `--title`, the title must also carry the conventional `!` marker. Without a base file there is nothing to compare, and the step passes with a note.

**Startup budget.** The new xtask task `check-startup --bin <toolu>` spawns `<bin> --version` for the configured warm-up and timed runs. It takes the nearest-rank p50 of the wall times, the same method as the hook bench's `percentile`, and compares it with `benchmarks/startup-budgets.json` (≤ 4 ms, 3 + 30 runs, from `docs/resource-budgets.md`). The Linux `rust` CI job runs it against the release binary it builds; macOS does not run it.

**Inventory.** `crates/cli/tests/inventory.rs` compares three name sets: the plugin manifests (`plugins/*/.claude-plugin/plugin.json`), the plugin crate directories (`crates/*` minus `core`, `cli` and `xtask`), and the owners in `toolu commands --json` (minus `toolu-cli`). It fails, naming the name and the side that lacks it, when the sets differ. Each plugin crate's unit test pins `PLUGIN` to its directory name.

**Reused:** #412's `HookRequest`, `hook::run`, `Context`, `output`, `HOOK_PROTOCOL`, `launcher::is_name` and `invocation::args`; xtask's `Options` (`--root`, `--base`, `--title`, `--bin`), `output::findings`, `gate` step runner, git helpers in `gate_change`, and `measure`'s spawn style.

## Interfaces / Schema

**Workspace** (`Cargo.toml`, product section): `clap = { version = "4.6", default-features = false, features = ["std", "help", "usage", "error-context", "suggestions"] }`; dev: `insta = { version = "1", default-features = false }`, `assert_cmd = "2"`, `jsonschema = { version = "0.58", default-features = false }`. Members gain `crates/toolu`, `crates/ts-quality`, `crates/python-quality`, `crates/rust-quality`, `crates/ast-grep`, `crates/brainstorm`, `crates/delivery-flow`, `crates/toolu-review`, `crates/jev`, `crates/statusline`, `crates/pr-babysit`, `crates/epic-orchestrator`. Package names are `toolu-<dir>` (`toolu-jev`, `toolu-ast-grep`, …). There are two exceptions: `toolu-review` keeps its name, and the hub is `toolu-hub`, because a library named `toolu` would collide with the `toolu` binary in `cargo doc`. `folders.json` `crates` gains the same twelve directory names.

**`toolu-protocol`**
```rust
pub mod exit;  // #[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Exit { Success, Failure, Blocked, Usage, Unavailable, TempFail }
               // impl Exit { pub const ALL: [Exit; 6]; pub fn code(self) -> u8 /* 0,1,2,64,69,75 */;
               //             pub fn name(self) -> &'static str /* success,failure,blocked,usage,unavailable,tempfail */;
               //             pub fn meaning(self) -> &'static str }
pub mod host;  // pub enum Host { Claude, Codex, Opencode, Cursor, Hermes }; Host::ALL, name() -> "claude"…, parse(&str) -> Option<Host>
```

**`toolu-runtime`** (gains `clap`)
```rust
pub mod cli;        // pub struct Ctx { pub json: bool, pub quiet: bool, pub host: Option<Host>, pub config_dir: Option<PathBuf> }
                    // pub struct Outcome { pub exit: Exit, pub stdout: Option<String>, pub stderr: Option<String> }
                    // impl Outcome { pub fn data(text: String) -> Self; pub fn failed(exit: Exit, message: String) -> Self }
pub mod namespace;  // pub const PLACEHOLDER: &str = "planned";
                    // pub struct Planned { pub name: &'static str, pub about: &'static str,
                    //                      pub verbs: &'static [&'static str], pub issues: &'static [u32] }
                    // impl Planned { pub fn command(&self) -> clap::Command; pub fn run(&self, m: &ArgMatches, ctx: &Ctx) -> Outcome }
                    // pub struct Guide { pub name: &'static str, pub about: &'static str, pub text: &'static str }
                    // impl Guide { pub fn command(&self) -> clap::Command; pub fn run(&self, m: &ArgMatches, ctx: &Ctx) -> Outcome }
```
`Planned::command()` is `<name>` with `about`, `subcommand_required` and `arg_required_else_help`, plus one verb, `planned`. That verb's about reads "Not ported yet (#n…): show the planned verbs". `Planned::run` gives `toolu <name> is not ported yet (#434, #435, #448). Planned verbs: engine, start, …` (or `Planned verbs: none (the command itself is planned)`), exit 0. With `--json` it gives `{"namespace":"epic","ported":false,"planned":["engine",…],"issues":[434,435,448]}`. `Guide::run` gives `text`, or `{"namespace":"brainstorm","about":…,"text":…}`, exit 0.

**Plugin crates** (each `src/lib.rs`, unit test in `src/tests/lib_test.rs`)
```rust
pub const PLUGIN: &str = "jev";
pub fn command() -> clap::Command;                    // Planned/Guide::command
pub fn run(matches: &clap::ArgMatches, ctx: &Ctx) -> Outcome;
```
Hub: `crates/toolu/src/{ledger,debug,setup,doctor,config,status,serve}.rs` each export `command`/`run`. `lib.rs` exports `PLUGIN = "toolu"` and `pub use toolu_ts_quality as ts_quality;` (the same for `python_quality`, `rust_quality` and `ast_grep`).

| Namespace | Owner | Kind | Planned verbs (issues) |
|---|---|---|---|
| `hook` | toolu | live (#412) | — |
| `ledger` | toolu | planned | run, status, preflight, path, root, self-test, verdict (#421) |
| `debug` | toolu | planned | io, log, stack, testfail (#425) |
| `setup` | toolu | planned | agents (#445) |
| `doctor` | toolu | planned | none (#445) |
| `config` | toolu | planned | get, set, validate (#445) |
| `status` | toolu | planned | none (#445) |
| `serve` | toolu | planned | none (#437) |
| `ts-quality` | ts-quality | planned | none (#426) |
| `python-quality` | python-quality | planned | none (#427) |
| `rust-quality` | rust-quality | planned | none (#428) |
| `ast-grep` | ast-grep | planned | search, files, scan, debug, savings (#429) |
| `brainstorm` | brainstorm | guide | — |
| `delivery-flow` | delivery-flow | guide | — |
| `review` (alias `toolu-review`) | toolu-review | planned | write-state, status (#432) |
| `jev` | jev | planned | noul, choice, score, ask (#430) |
| `statusline` | statusline | planned | render, setup, refresh (#431) |
| `babysit` (alias `pr-babysit`) | pr-babysit | planned | tick, collect, record, reply, resolve, route-fix, dispatch-fix (#433) |
| `epic` (alias `epic-orchestrator`) | epic-orchestrator | planned | engine, start, status, pause, resume, ack, answer, wait, report, job, graph, route, launch, finish, close, release, jira, probe, gate, queue (#434, #435, #448) |
| `commands` | toolu-cli | live | — |
| `plugins` | toolu-cli | planned | install, list, remove, update (#438) |

**Root command** `toolu`. It has `-V/--version` (prints `toolu <version>`), `-h/--help`, and the global flags `--json`, `-q/--quiet`, `--host <claude|codex|opencode|cursor|hermes>` and `--config-dir <DIR>`, plus a hidden `--hook-protocol`. Its `after_long_help` documents the streams and the exit-code table. `toolu` alone prints help on stderr and exits 64.

**`hook`** (top level, and hidden in each plugin namespace): `hook <NAME> [--event <EVENT>] [--plugin-root <DIR>]`, the grammar #412's fast path accepts.

**`commands`**: `toolu commands` prints one line per visible command (`toolu <path>` padded, then its about). `--json` prints the tree. `--schema` prints the JSON Schema.

**`toolu.commands/v1`** (`additionalProperties: false` throughout):
```json
{ "schema": "toolu.commands/v1", "name": "toolu", "about": "…", "hookProtocol": 1,
  "exitCodes": [{ "code": 0, "name": "success", "meaning": "…" }],
  "flags": [Flag], "commands": [Command] }
Command  = { "name", "path": [string], "about", "longAbout": string|null, "aliases": [string],
             "owner": string, "hidden": bool, "placeholder": bool,
             "flags": [Flag], "args": [Positional], "commands": [Command] }
Flag     = { "long": string, "short": string|null, "valueName": string|null, "takesValue": bool,
             "required": bool, "global": bool, "hidden": bool, "possibleValues": [string], "help": string }
Positional = { "name": string, "required": bool, "multiple": bool, "help": string }
```
The tree omits the implicit `--help` on every command. Global flags appear once, at the root. Order is declaration order.

**Other `--json` documents** (the schema's `$defs`):
- `error`: `{"error":{"code":64,"name":"usage","message":"…","suggestion":"epic"|null}}`
- `version`: `{"name":"toolu","version":"7.11.0","hookProtocol":1}`
- `help`: `{"help":"…"}`
- `planned`, `guide`: as above
- `empty`: `{}`

**xtask**
- `docs-cli [--check] [--bin FILE] [--root DIR]`. Without `--bin` it runs `cargo build --locked -p toolu-cli --message-format=json` and takes the `toolu` executable from the artifact line. `--check` is a value-less option.
- `check-cli-compat [--base REF] [--title TEXT] [--root DIR]`.
- `check-startup --bin FILE [--root DIR]`.
- Gate steps `docs-cli` and `cli-compat` run after `unused-pub` and before `jscpd`.
- `inventory.json` gains a pass and a fail test per new task.
- `benchmarks/startup-budgets.json`: `{"version":1,"command":["--version"],"warmup":3,"runs":30,"percentile":50,"wallMs":4}`, read strictly (`deny_unknown_fields`).
- Generated-file marker: `<!-- Generated by cargo xtask docs-cli from toolu commands --json and --help; do not edit. -->`.

**CI**
- `.github/ci-paths.json`: the `rust` group gains `docs/cli/**` and `benchmarks/startup-budgets.json`.
- `tests.yml`: the Linux `rust` job runs `cargo xtask check-startup --bin <release toolu>` after its release build.

## Failure modes and edge cases

- **Typo** (`toolu epik start`): exit 64. Stdout is empty. Stderr has clap's error with `tip: a similar subcommand exists: 'epic'`. With `--json`, stdout is the `error` document with `"suggestion":"epic"`, and stderr still has the message.
- **Missing verb** (`toolu epic`): help on stderr, exit 64. The same happens for `toolu` alone.
- **`--help` / `--version`**: stdout, exit 0. With `--json`, the `help` or `version` document.
- **Unknown `--host` value**: exit 64. Clap lists the possible values.
- **Malformed hook line** (`toolu hook pre-tools --bogus x`): the fast path declines and clap rejects it, exit 64. An enforcing launcher maps that to 2 (#412). `toolu hook --help` reaches clap and prints help, exit 0.
- **Hook with no native implementation** (`toolu hook pre-tools --event PreToolUse`): exit 2, `blocked: …` on stderr (#412, unchanged). That is the "denies → exits 2" scenario. A context event exits 0 with a `systemMessage`.
- **`--json` on hooks**: ignored. Hooks speak the host protocol.
- **A verb that fails without stdout under `--json`**: `crates/cli` synthesizes the `error` document from the exit and the stderr text.
- **Closed stdout or stderr**: the write is dropped (#412). The exit code is unchanged.
- **Non-UTF-8 argv**: replaced lossily (#412).
- **`docs-cli --check` with an absent `docs/cli/`**: every file is reported missing, exit 1. A hand-edited marker page is reported stale. A marker page for a removed command is reported orphaned. In write mode the generator deletes orphaned marker pages and never touches unmarked files.
- **`docs-cli` when cargo build fails or the binary exits non-zero**: setup error, exit 2, with the command and its stderr.
- **`check-cli-compat` without `docs/cli/commands.json` at the merge base**: clean, with a note.
- **`check-cli-compat` with unreadable JSON on either side**: setup error, exit 2.
- **`check-cli-compat` when `hookProtocol` decreased**: a finding.
- **`check-cli-compat` when a breaking change has a bumped `hookProtocol` but a title without `!`**: a finding.
- **`check-startup` with a missing or non-executable binary, a non-zero exit, or output not starting with `toolu `**: setup error, exit 2.
- **`check-startup` with a p50 over budget**: exit 1, naming p50, p90 and the budget.
- **`check-startup` with an invalid budget file**: exit 2.
- **Inventory** with a manifest without a crate, a crate without a manifest, or an owner without either: the test fails and names each.

## Acceptance criteria

- **AC-1:** `toolu --help`, `toolu --version` and `toolu <ns> --help` exit 0 with output on stdout and nothing on stderr. `<ns>` is every visible top-level command in `toolu commands --json`, which includes all twelve plugins' namespaces (`brainstorm` and `delivery-flow` among them) and the two built-ins. Every unported namespace's help lists the `planned` verb.
- **AC-2:** `toolu <ns> planned` prints that namespace's planned verbs and issues, and with `--json` emits one `planned` document that validates against the schema. `toolu brainstorm` and `toolu delivery-flow` print their guide.
- **AC-3:** The inventory test passes on the repository: 12 manifests ↔ 12 plugin crates ↔ 12 plugin owners in `toolu commands --json`, `brainstorm` and `delivery-flow` included. It fails, naming the missing name and the side that lacks it, when any one side loses an entry.
- **AC-4:** `toolu commands --json` validates against `toolu commands --schema`, and an insta snapshot pins its exact text.
- **AC-5:** Black-box tests pin the exit codes and streams:
  - `toolu epik start` exits 64 with an empty stdout and an `epic` suggestion on stderr;
  - `toolu --json epik start` exits 64 with one `error` JSON document on stdout that names `epic`;
  - a missing verb exits 64 with help on stderr;
  - `toolu hook pre-tools --event PreToolUse` exits 2 with `blocked:` on stderr;
  - a successful verb exits 0 with an empty stderr;
  - the scenario `toolu epic status 402 --json` (`--json` after the verb) exits 64 with exactly one `error` JSON document on stdout and nothing else, until #434 ports the verb;
  - `toolu --host bogus commands` exits 64 and names the possible values on stderr; `--host codex` and `--config-dir DIR` are accepted;
  - `--json` help and version are single JSON documents;
  - every `--json` document above validates against its `$defs` entry in `toolu commands --schema`;
  - each case asserts both streams.
  
  No #442 command produces 1, 69 or 75. Unit tests pin those codes in `Exit`, the error envelope built for them, and `--quiet` dropping a success-path stderr line while keeping an error's.
- **AC-6:** The fast path builds no tree. A unit test runs `toolu hook <name> …`, `toolu <plugin> hook <name> …` and `toolu --hook-protocol` with a tree builder that records calls, and the builder is never called. `toolu --version` calls it exactly once.
- **AC-7:** The clap tree accepts every hook form #412 generates. `toolu jev hook --help` and `toolu pr-babysit hook --help` exit 0, and `toolu commands --json` lists those hidden `hook` verbs and the plugin-name aliases.
- **AC-8:** `cargo xtask docs-cli` writes `docs/cli/` from the real binary. `cargo xtask docs-cli --check` (gate step `docs-cli`) passes on the committed tree, and fails, naming the file, on a stale, missing or orphaned generated file.
- **AC-9:** `cargo xtask check-cli-compat` passes on this branch against `origin/main`, where the base has no tree. It fails on a removed verb, alias, flag, short flag, possible value or exit code, and on an optional argument that became required. It passes when only a placeholder disappears or when `hookProtocol` increased and the title has `!`.
- **AC-10:** `cargo xtask check-startup --bin target/release/toolu` reports a p50 wall within 4 ms (3 warm-up, 30 runs, nearest-rank). It fails, naming the budget, for a binary slower than the budget, and the Linux `rust` CI job runs it against the release binary. Startup does no namespace work: every `command()` only constructs clap values, `run` executes only for the matched namespace, and `--version` is answered by clap before any `run`.
- **AC-11:** `cargo xtask gate` passes in full with the new crates, steps and tasks, with no exemption. `bun run test` passes apart from the documented environmental failures that also fail on `origin/main`.
- **AC-12:** `docs/cli.md` no longer exists; its guide is `docs/cli/installer.md`, every reference resolves, and the OpenCode surface drift check passes after regeneration.

## Acceptance evidence

| AC | Real input | Expected | Boundary / failure | Check |
|---|---|---|---|---|
| AC-1 | the built `toolu` and every visible top-level command in its own `commands --json` | exit 0, stdout has `Usage: toolu <ns>`, stderr empty | an unported namespace shows `planned`; the 14 expected names are all present | `cargo test -p toolu-cli --test contract` (assert_cmd) |
| AC-2 | `toolu epic planned`, `toolu --json jev planned`, `toolu brainstorm` | verbs and issues text; a `planned` document valid against the schema; guide text | `doctor` (no planned verbs) says so | `cargo test -p toolu-cli --test contract`, `--test commands` |
| AC-3 | the repository's `plugins/`, `crates/` and the binary's tree | three equal sets | each side with one name removed → names it | `cargo test -p toolu-cli --test inventory` |
| AC-4 | `toolu commands --json` and `--schema` | valid; snapshot `crates/cli/tests/fixtures/commands__commands_json.snap` matches | an edited document fails validation (a test drops a required key) | `cargo test -p toolu-cli --test commands` |
| AC-5 | the binary run by assert_cmd with the typo, `toolu epic status 402 --json`, missing-verb, hook, `--host bogus` and `--json` help/version argv | codes and both streams as stated; JSON documents valid against the schema | `--json` usage error after the verb; 1/69/75 and `--quiet` in unit tests | `cargo test -p toolu-cli --test contract`; `cargo test -p toolu-protocol`; `cargo test -p toolu-cli --bin toolu` |
| AC-6 | `run(words, ctx, &recording_tree)` in-process | 0 builds for the fast forms, 1 for `--version` | malformed hook line → 1 build (clap) | `cargo test -p toolu-cli --bin toolu` |
| AC-7 | `toolu jev hook --help`, `toolu pr-babysit hook --help`, the tree JSON | exit 0; hidden hook verbs and aliases present | `toolu hook --help` → clap help | `cargo test -p toolu-cli --test contract` |
| AC-8 | the real `toolu` built by cargo, a temp `--root` | write then `--check` → 0; `--check` on an empty root → 1 naming files; an orphaned marker file → 1 | an unmarked `installer.md` is untouched | `cargo test -p xtask --test docs_cli`; `cargo xtask docs-cli --check` |
| AC-9 | `docs/cli/commands.json` produced by the binary, then edited copies in a temp git repo | removed verb/alias/flag/short/value/exit code → 1; optional → required → 1; placeholder removal → 0; bump + `feat!:` title → 0; bump without `!` → 1 | no base file → 0 with a note | `cargo test -p xtask --test cli_compat`; unit tests on the comparison |
| AC-10 | the release `toolu`; a script that sleeps past the budget | p50 ≤ 4 ms → 0; slow script → 1 naming the budget | a binary that prints the wrong text → 2 | `cargo test -p xtask --test startup`; `cargo xtask check-startup --bin target/release/toolu`; the `tests.yml` step on `ubuntu-latest` |
| AC-11 | the whole workspace | every gate step ok | — | `cargo xtask gate`; `bun run test` |
| AC-12 | the repository after the move | no `docs/cli.md`; references resolve; surface drift clean | — | `bun run check:opencode-docs`, `bun run check:opencode-surface`, `bun run test:docs` |

## Documentation impact

- `docs/cli/`: generated `README.md`, one page per command, `commands.json` and `commands.schema.json`.
- `docs/cli/installer.md`: moved from `docs/cli.md`, with a line pointing at `README.md` for the Rust CLI.
- `AGENTS.md`: the Tech-stack CLI bullet (Rust `toolu` plus the Node installer) and the Plugin layout, which now describes a plugin as a crate (a library with one namespace) plus Markdown. Also Key files (`crates/cli/src/main.rs`, `registry.rs`, `docs/cli/`), the Rust conventions commands (`docs-cli`, `check-cli-compat`, `check-startup`), the CI paths of the `rust` group, and the Contributing steps for a namespace change (regenerate docs, accept the snapshot).
- `tooling/templates/plugin-README.md`: a crate and namespace section.
- `docs/resource-budgets.md`: the startup row's measurement and gate.
- `README.md`, `docs/opencode.md`, `docs/plugins/index.md`, `tools/toolu-cli/npm/README.md` and `tooling/src/check-opencode-docs.ts`: references to `docs/cli/installer.md`.
- The regenerated OpenCode surface.

## Open Questions

- **When does PR #476 merge?** Owner: the orchestrator. Non-blocking for the spec and plan. Execution starts on `origin/main` after it merges. If it has not merged by then, the plan's first steps (plugin crates, runtime and protocol types, xtask tasks) do not touch #476's files.
- **The `cli-verb` inventory kind.** Owner: a later `chore(gates):` PR (#444 or the orchestrator). Non-blocking; see Non-Goal 5.

## Spec review

Round 1 (Needs changes), findings fixed in place:

- Acceptance evidence: 🟡 should-fix: the issue scenario `toolu epic status 402 --json` was not exercised. Added to AC-5 with `--json` after the verb.
- Acceptance evidence: 🟡 should-fix: the contract tests did not say they were assert_cmd tests asserting both streams. Named in AC-5 and its evidence row.
- Acceptance criteria: 🟡 should-fix: `--host`, `--config-dir` and `--quiet` had no AC. Added to AC-5 (black-box for `--host`/`--config-dir`, unit test for `--quiet`).
- Acceptance criteria: 🟡 should-fix: the startup AC had no CI gate and no statement on namespace work at startup. AC-10 now names the Linux CI step and the pure `command()` rule.
- Acceptance criteria: 🔵 consider: the inventory and help ACs did not name `brainstorm` and `delivery-flow`, and AC-9 omitted short flags, exit codes and optional → required. Added.

Jev (`jev-1.13.0`, `--raw`): coverage of the issue's acceptance items and scenarios.
- Round 1: `acc_help` 0.46, `acc_exit` 0.72, `acc_startup` 0.63, `scen_json` 0.19; `acc_json`, `acc_docs`, `acc_fast`, `scen_typo` and `scen_hook` ≥ 0.86.
- After the fixes: `acc_exit` 0.94, `acc_startup` 0.91 and `scen_json` 0.78.
- `acc_help` stayed at 0.47 under its "at least one AC" framing. Split into its two parts, it scored 0.92 (help) and 0.96 (inventory), with 0.33 that any requirement is unaddressed.

Round 2: Approved.
