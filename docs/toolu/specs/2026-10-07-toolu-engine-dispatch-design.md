# toolu-engine: registry runner and dispatch — Design

**Date:** 2026-10-07   **Status:** Draft   **Author:** epic worker (#418)   **Topic:** port `@toolu/core/dispatch` and `@toolu/core/registry` to `crates/core/engine`, with compiled-in rules, executable modules and a temporary Bun bridge

## Problem

Every tool call runs toolu's PreToolUse and PostToolUse dispatchers. Today they are Bun bundles costing 48–56 MiB and 91–166 ms CPU per spawn on Linux CI (`docs/resource-budgets.md`). Epic #402 moves them to the `toolu` binary. The gates (#419–#423) and the rule crates (#426–#429) all plug into one walk: built-ins in table order, then the registry. That walk does not exist in Rust yet. Until it does, no gate can be ported against the real ordering, precedence, per-path patch handling and third-party module contract that users and plugins depend on.

## Non-Goals

1. No built-in gate is ported. The built-in tables are empty and #419–#423 fill them.
2. `hooks.json` keeps running the Bun bundles, and `fixtures/rust-ported.json` stays empty. No host reaches the native `toolu hook pre-tools` or `post-tools` before #425.
3. The TypeScript dispatcher and registry are not changed, apart from one new test that replays the shared golden.
4. No new module kind is admitted. The registry reads `.js`, `.sh` and `.json` (`toolu_runtime::registry::parse_name`). Arbitrary executables and the executable protocol of #440 are out of scope.
5. The manifest format of #414 does not change. No plugin writes manifests yet, since that starts with #426.
6. The `mcp__` hook (`mcp-tools`) and agent-tier are not wired; #422 owns them.
7. No resource budget changes.

## Architecture

The walk is a port of `dispatch.ts`, `dispatch-walk.ts`, `dispatch-output.ts`, `dispatch-bash.ts`, `registry-run.ts`, `registry-list.ts`, `registry-gate.ts` and `registry-prune.ts`, under `crates/core/engine/src/`:

- `dispatch` holds the two entry points. `dispatch_pre_tool` and `dispatch_post_tool` are ports of `dispatchHook`. They read the config switch `hooks.pre-tools` / `hooks.post-tools`, then build the session (`sessionFor`), then split the edit records (`dispatchInput`, `dispatchRecords`).
- `dispatch/walk` is one walk over one payload. Built-ins come first, then the registry. `consume` and `settle` decide when the walk ends: exit 2, a deny before the tool, a block after it. They also hold the first ask and merge advisories.
- `dispatch/output` holds the jq-compatible readers and printers: `readField`, `finalAsk`, `finalAdvisory`. They print over `toolu_runtime::json::ordered::Ordered`, reordered as `JSON.parse` orders keys (`toolu_state::js_order`, made public), so the bytes match TypeScript's `toJqJson`.
- `registry` covers the directory listing, installed-plugin gating (Claude `installed_plugins.json`, the Codex snapshot through `toolu_runtime::host::snapshot`), shadowing and the Codex prune.
- `registry/executable` runs a `.sh` module as `bash <path>` through `toolu_runtime::process::run`, the only spawner (rule 14). It adds a deadline and output isolation.
- `registry/bridge` is the temporary Bun bridge for `.js` modules. Its runner is an embedded JavaScript string run with `bun -e`.
- `detect` holds `is_git_push`, `is_git_commit`, `push_target_root` and `push_target_branch`, on top of `toolu_shell::git` and the `.git` reads of `toolu_runtime::git` / `toolu_state::git`. Nothing is spawned unless the walk defers to git.
- `builtins` holds the two empty built-in tables (`PRE_TOOL`, `POST_TOOL`) that #419–#423 append to.

The decisive trade-off: consecutive `.js` modules share one Bun process (brainstorm, Jev 0.88). Order and stop-after-deny stay exact, and today's four quality modules cost one Bun start instead of four. A timeout or crash isolates only the module in flight.

The CLI wiring is thin. `crates/toolu` (the hub, which owns rule crates) gets `tool_hook`. It reads the process environment and working directory, picks the engine's built-in table and the hub's (empty) rule table, and runs the engine inside `toolu_protocol::hook::run_hook_io` over in-memory streams. A panic therefore exits 2 with `blocked: toolu PreToolUse hook panicked: …`. `crates/cli/src/hook.rs` routes `toolu hook pre-tools` and `toolu hook post-tools` to it after the existing skew prelude.

Reused as is:
- runtime: `Env`, `Roots`, `config::load` and `config::read::enabled`, `host::detect`, the `registry` types, `ModuleManifest` and `read_manifest`, `Rule` and `RuleContext`, and `json::{jq_text, ordered}`;
- protocol: `decision::Decision`, `encode::encode`, `hook::{run_hook_io, Reply::Raw}`;
- state: `edit_records::normalize_edit_records` and `git::current_branch`;
- shell: `git::{runs_git_subcommand, push_targets}`.

## Interfaces / Schema

### Rust (`toolu-engine`)

```rust
/// A built-in gate (`ToolModule` in dispatch-context.ts): run in process before the registry.
pub trait Gate: Sync {
  fn name(&self) -> &str;
  /// `Err(message)` is a thrown error: `<message>` on stderr, then the module is skipped as exit 1.
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String>;
}

pub mod builtins { pub const PRE_TOOL: &[&dyn Gate] = &[]; pub const POST_TOOL: &[&dyn Gate] = &[]; }

pub struct DispatchOptions<'a> {
  pub env: &'a Env,                       // the hook's environment (HostEnv)
  pub cwd: &'a Path,                      // the hook process's working directory
  pub lib_dir: &'a Path,                  // exported to .sh modules as TOOLU_LIB_DIR
  pub builtins: &'a [&'a dyn Gate],
  pub rules: &'a [&'a dyn Rule],          // compiled-in rules a manifest may enable
  pub selected_specs: Option<&'a BTreeSet<String>>,  // selectedRegistrySpecs
  pub continue_post_blocks: bool,         // continuePostBlocks
  pub module_timeout: Duration,           // per .sh module, per Bun batch; DEFAULT_MODULE_TIMEOUT = 30 s
}

pub struct ModuleResult { pub stdout: String, pub stderr: String, pub exit_code: i32 }  // exit 0 or 2

pub struct Dispatched { pub result: ModuleResult, pub trace: Vec<Step> }
pub struct Step { pub module: String, pub kind: StepKind, pub status: StepStatus }
pub enum StepKind { Builtin, Rule, Executable, Esm }
pub enum StepStatus { Decided, Exited(i32), Failed(String), Skipped(Skip) }
pub enum Skip { Inactive, Shadowed, NotMatching, NoBun }

pub fn dispatch_pre_tool(stdin: &str, options: &DispatchOptions<'_>) -> Dispatched;
pub fn dispatch_post_tool(stdin: &str, options: &DispatchOptions<'_>) -> Dispatched;

// registry
pub struct Entry { pub file: String, pub path: PathBuf, pub spec: String, pub name: String, pub kind: ModuleKind }
pub struct Listing { pub entries: Vec<Entry>, pub rejected: Vec<String> }
pub fn list_dir(dir: &Path) -> Listing;
pub enum Presence { Installed, Absent, Unknown }
pub fn plugin_presence(spec: &str, roots: &Roots) -> Presence;
pub fn plugin_active(spec: &str, roots: &Roots) -> bool;
pub fn prune_inactive_modules(roots: &Roots) -> Vec<PathBuf>;

// detect
pub fn is_git_push(analysis: &ShellAnalysis) -> bool;
pub fn is_git_commit(analysis: &ShellAnalysis) -> bool;
pub fn push_target_root(analysis: &ShellAnalysis, roots: &Roots, cwd: &Path) -> PathBuf;
pub fn push_target_branch(analysis: &ShellAnalysis, root: &Path, env: &Env) -> String;
```

`toolu-runtime` gains two additive items:
- `process::Spec::wait: Wait`. It is `Group`, today's behaviour and the default, or `Streams`: return once the child has exited and both of its streams have closed, as `Bun.spawnSync` does, leaving any process it backgrounded alone. The deadline still terminates the whole group.
- `invocation::current_dir() -> PathBuf`, the process working directory, else `.`.

`toolu-state` makes `js_order` public.

### Walk semantics (byte-exact with TypeScript)

- **Order:** built-ins in table order, then the registry entries in byte order of file names. Dotfiles, names that are not modules and non-files (after following symlinks) are ignored. Un-namespaced module files are listed as rejected and produce one line each, `toolu-registry: registry module <file> lacks <plugin-spec>__<name> namespace; skipped`.
- **Gating:** an entry runs unless its plugin is definitively absent: not listed in Claude's `installed_plugins.json` (`CLAUDE_PLUGINS_REGISTRY`, else `<TOOLU_CONFIG_DIR | CLAUDE_CONFIG_DIR | ~/.claude>/plugins/installed_plugins.json`), or absent from a ready Codex snapshot. With `selected_specs` set, an entry also needs its spec in the set. An unreadable record fails open. Other hosts count as installed. The answer is memoized per spec per walk.
- **Shadowing:** a `.sh` entry is shadowed when any `.js` entry in the directory has its spec. A spec with at least one usable manifest (valid, and naming a rule in `rules` for this event) shadows that spec's `.js` and `.sh` entries.
- **Manifests:** a manifest is read with `read_manifest`. Any problem gives one line, `toolu-registry: manifest <file>: <reason>; skipped`, and is never an error. The problems are: unreadable, an unknown field, a `version` other than the supported 1, a spec, name or event that does not match the file, an empty matcher entry, or no compiled-in rule of that spec and name for this event (`names no rule in toolu <VERSION>`). A usable manifest whose matcher does not fit the walk's tool name is `Skipped(NotMatching)`, and its rule code is not called. A fitting one calls `Rule::run` and is consumed like an ESM decision.
- **`.sh` contract** (`dispatch-walk.ts:90-103`, `dispatch-bash.ts`):
  - argv is `bash <path>`, cwd is the hook's cwd, and stdin is the payload text plus `\n`.
  - The environment is the session environment plus `input` (the payload text), `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR`, and, for a split patch path, `TOOLU_EDIT_OPERATION`, `TOOLU_EDIT_FROM` and `TOOLU_EDIT_MOVED_TO`.
  - The status is the exit code, or 128 plus the signal number. Stdout has every trailing newline stripped.
  - Exit 2 ends the walk: stdout is empty, stderr is the walk's stderr so far plus the module's, and the hook exits 2.
  - Any other non-zero status adds `toolu-dispatch: module <file> exited <status>; output skipped`.
  - The wait is `Streams`. At the deadline the process group is killed and the status is fixed at **124**, so the same line is printed and a timed-out module can never deny.
  - A spawn failure is status 127.
- **Decisions:** a module's stdout is read like `jq -r '.<path> // empty'`. Before a tool, the first `permissionDecision: "deny"` is printed verbatim plus `\n` and ends the walk. The first `ask` is held. After a tool, the first `decision: "block"` does the same and `permissionDecision` is ignored. `additionalContext` and `systemMessage` are each deduplicated by exact text and joined with a blank line. The held ask gets the joined contexts appended to `permissionDecisionReason` and the joined messages appended to `systemMessage`, and is printed like `jq -n`, falling back to the raw ask. Without an ask, the merged advisory is printed like `jq -n` with `hookEventName` set to `PreToolUse` or `PostToolUse`.
- **Built-ins, rules and `.js`:** their decisions are encoded with `toolu_protocol::encode::encode`. Codex is encoded as Codex, every other host as Claude, as in `dispatch-walk.ts:111`. A built-in `Err(message)` adds `<message>\n`, then the `exited 1` line.
- **Edits:** an edit tool is split with `normalize_edit_records`. Each path walks a synthetic `Edit` payload, `jq -c` of the payload with `tool_name: "Edit"` and `tool_input` extended with `file_path`, `path` and `toolu_edit_operation`, `toolu_edit_from` and `toolu_edit_moved_to`, all in TypeScript key order. A deny, block or exit 2 on any path ends the whole patch. A malformed patch, or one with no records, prints the fixed `MALFORMED_PATCH_DENY` or `MALFORMED_PATCH_BLOCK`. With `continue_post_blocks`, every path's block reason is collected and printed as one `{"decision":"block","reason":"<r1>\n\n<r2>"}`, and an exit 2 still ends at once.
- **Session:** before a tool, `TOOLU_CONFIG_DIR` is added and the project root is `Roots::project_root(cwd)`, else the cwd. After a tool, `PROJECT_ROOT` is the git toplevel of the cwd, else the cwd, and `PATH` gains `<PROJECT_ROOT>/node_modules/.bin:` in front. `hooks.<pre|post>-tools: false` makes the hook print nothing. Config warnings come first on stderr as `toolu-config: <line>`.

### Bun bridge protocol

- Bun is resolved from `TOOLU_BUN` (a file that is executable), then `bun` on the session `PATH`, then `$HOME/.bun/bin/bun`.
- A batch is a maximal run of consecutive active `.js` entries in walk order. It is spawned as `[bun, "-e", RUNNER]`, with the session environment and the hook's cwd.
- Stdin carries one JSON request:
  ```json
  {"stop":"deny"|"post_block","registryEvent":"tool/pre"|"tool/post",
   "event":<NormalizedEvent wire: type, sessionId, cwd, projectRoot, worktree, toolCallId, toolName, toolInput, command?, toolOutput?>,
   "ctx":{"host":…,"configRoot":…,"projectRoot":…,"cwd":…,"raw":<payload object>,"edit"?:{"operation","from","movedTo"}},
   "modules":[{"file":…,"path":…,"spec":…,"name":…}]}
  ```
- The runner sets `ctx.env = process.env`. For each module in order it imports the path and checks the default export the way `runContract` does, then calls `run.call(exported, event, ctx)` and validates the result the way `DecisionSchema` does. It writes one line `toolu-bridge:{"file":…,"decision":{…}}` or `toolu-bridge:{"file":…,"error":"<message>"}`, using TypeScript's exact messages:
  - `default export is not a registry module`;
  - `contract mismatch: exports {…}, file and directory want {…}`;
  - `invalid decision: <inspect(result)>`;
  - the thrown message.

  After a decision whose kind is `stop`, it exits.
- The engine reads only the marked lines, in order. An error line becomes `toolu-registry: module <file> failed: <error>; output skipped`. A decision is parsed strictly and consumed. Bun's stderr is appended to the walk's stderr.
- If the batch ends before every module has a line, the first module without one fails with `bridge exited <status>`, `timed out after <N>s` or `bridge output unreadable`, and the modules after it run in a new batch.
- A deadline is `module_timeout` per batch.
- **No Bun, or Bun cannot be spawned:**
  - Before a tool: the first applicable `.js` module is a deny (`toolu-registry: module <file> needs Bun 1.4.x, which was not found (checked TOOLU_BUN, PATH and ~/.bun/bin/bun); install it from https://bun.sh or remove the module`), and the walk stops. Its step is `Skipped(NoBun)`.
  - After a tool: every `.js` module is skipped, and the walk adds one advisory, `toolu-registry: <n> registry module(s) did not run because Bun was not found: <files>. Install Bun 1.4.x from https://bun.sh.` The advisory is shown only when no marker exists at `<project state root>/registry-bridge/<sanitized session_id>`. That marker is created exclusively when the advisory is shown, so the advisory appears once per session.

### Shared fixture: `fixtures/dispatch/cases.json`

```json
{ "version": 1, "cases": [ {
  "name": "…", "phase": "pre"|"post", "host": "claude"|"codex",
  "stdin": <object, encoded with JSON.stringify> | "<raw text>",
  "config": <toolu.config.json object>?,
  "builtins": [ {"name": "…", "decision": <Decision>} | {"name": "…", "throws": "<message>"} ]?,
  "registry": [ {"file": "<name>", "sh": "<bash body>"} | {"file": "<name>", "js": "<ESM source>"} ]?,
  "installed": ["<spec>", …]?,   // Claude installed_plugins.json, or a ready Codex snapshot; absent: no record
  "continuePostBlocks": true?,
  "expect": {"stdout": "…", "stderr": "…", "exitCode": 0|2}
} ] }
```

`$PROJECT`, `$HOME`, `$CONFIG` (the host's config root) and `$LIB` expand in `stdin` strings, module bodies and `expect`. The sandbox holds a git `project`, a `home`, a Codex home and a plugin `lib` path. The cwd is `$PROJECT`, and on Claude `CLAUDE_PROJECT_DIR` is `$PROJECT`. `expect` is captured once from the TypeScript dispatcher and committed. `packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts` (TypeScript) and `crates/core/engine/tests/dispatch_fixture.rs` (Rust) both reproduce it exactly. The suite is listed in `fixtures/index.json` (`dispatch-cases`) and in `fixtures/README.md`.

### CLI

`toolu hook pre-tools` and `toolu hook post-tools` take the existing flags `--event` and `--plugin-root`. `TOOLU_LIB_DIR` is `<plugin root>/hooks/lib`, where the plugin root is `--plugin-root`, else the host's plugin-root variable. Stdout is the dispatcher's stdout. Stderr is its stderr; when that lacks a final newline, the CLI's line writer adds one. The exit is 0 or 2.

## Failure modes and edge cases

| Input | Behaviour | Propagation |
|---|---|---|
| Stdin is not JSON | `tool_name` reads as `""`, which is not an edit, so the raw text walks once and modules receive it unchanged | recovered, TypeScript parity |
| Absent registry directory | no entries, nothing spawned | recovered |
| `installed_plugins.json` unreadable or malformed, or the Codex snapshot missing or stale | the plugin counts as installed (fail open, as TypeScript) | recovered |
| `.sh` exit 2 | hard deny: hook exit 2, the module's stderr | propagates |
| `.sh` killed by a signal (crash) | `exited <128+n>; output skipped`, the walk goes on | converted |
| `.sh` passes the deadline, including one that traps SIGTERM and exits 2 | group killed, `exited 124; output skipped`, the walk goes on | converted |
| `.sh` backgrounds a process with its streams redirected | the walk does not wait for it | recovered (spawnSync parity) |
| `.sh` stdout not JSON, or JSON without decision fields | no effect | recovered |
| `.sh` stdout over 8 MiB | output past the budget is dropped, so a cut-off document is unparseable and has no effect | converted |
| `bash` missing | status 127, `exited 127; output skipped` | converted |
| `.js` import error, throw, contract mismatch, invalid decision | `toolu-registry: module <file> failed: …; output skipped` | converted |
| `.js` module calls `process.exit` or hangs | that module fails (`bridge exited …` or `timed out after …`), and the rest re-run in a new batch | converted |
| Bun missing before a tool, with an applicable `.js` | deny naming the module | fail closed |
| Bun missing after a tool | one advisory per session; the modules are skipped | converted |
| Manifest problems | one `toolu-registry: manifest …; skipped` line | converted |
| Rule or gate panics | caught by `run_hook_io`: `blocked: toolu PreToolUse hook panicked: …`, exit 2 (post: same line without `blocked:`, exit 2) | fail closed |
| `apply_patch` headers unparseable | the fixed malformed deny or block | fail closed |
| A lone surrogate escape in the payload, which `JSON.parse` accepts and serde rejects | treated as not JSON: no split, modules get the raw text | documented deviation |
| Module stderr without a final newline, at the CLI | the CLI adds one newline (engine output is exact) | documented deviation |

## Acceptance criteria

- **AC-1:** Given each case in `fixtures/dispatch/cases.json`, the Rust engine produces exactly the captured stdout, stderr and exit code. That includes precedence (deny over ask over advisory, block over advisory), byte-order module ordering, per-path patch results, the malformed-patch replies, `continuePostBlocks`, gating, the Codex ask degradation of built-ins and the config switch. The TypeScript dispatcher reproduces the same file.
- **AC-2:** Given a third-party `.sh` module that exits 2 after reading stdin and its environment, the tool is blocked (exit 2) with the module's stderr. The module received the payload plus `\n` on stdin and `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and `TOOLU_EDIT_*` exactly as TypeScript gives them (fixture cases).
- **AC-3:** Given a `.sh` module that sleeps past the deadline (also one that traps TERM and exits 2), or one killed by SIGSEGV, the walk reports `toolu-dispatch: module <file> exited 124|139; output skipped`, never denies, and the modules after it still run.
- **AC-4:** Given `x@t__a.sh` and `x@t__b.js` of the same spec, only the `.js` runs and the `.sh` step is `Skipped(Shadowed)`. Given `noname.sh`, it never runs and stderr carries the namespace line. Both match TypeScript's fixture output.
- **AC-5:** Given a `Read` call and a registry holding only manifests whose matchers do not fit `Read`, no rule's `run` is called, the trace has no `Decided`, `Exited` or `Failed` step, and no process is spawned: sentinel `bash` and `bun` executables first on `PATH` are never run. The same holds for the release `toolu hook pre-tools`, run through `cargo xtask measure`, which is recorded in the PR.
- **AC-6:** Given a `.js` module that is a real committed quality bundle (`plugins/ts-quality/hooks/dist/post-tool-use.js`, registered as `ts-quality@toolu__ts-quality.js`) in a TypeScript project (tracked `tsconfig.json`, `bun.lock`, Bun on `PATH`) and an edit to a `.ts` file with a violation, the Rust post-tool dispatch prints the same stdout, stderr and exit code as the TypeScript dispatcher run live on an identical sandbox, and both record the same gate entry.
- **AC-7:** Given Bun absent (`TOOLU_BUN` unset, no `bun` on `PATH`, no `~/.bun/bin/bun`) and an active pre-tool `.js` module, the pre-tool dispatch denies, naming the module. Given the same for post-tool, the first call in a session prints the advisory once, and a second call with the same `session_id` prints none.
- **AC-8:** Given three consecutive `.js` modules, one Bun process runs them. A sentinel `bun` wrapper logs one spawn, and results come in byte order with the walk stopping after a deny. Given a middle module that hangs or calls `process.exit(3)`, that module alone fails and the third still runs.
- **AC-9:** Given `ts-quality@toolu__ts-quality.json` (a usable manifest for a test rule matching `Edit|Write`) and an edit to a `.py` file whose rule only owns `.ts`, the rule decides allow. Given a manifest whose matcher is `Write` and an `Edit` call, the rule is not run. Given a manifest with `version: 2`, an unknown field or no compiled-in rule, one `toolu-registry: manifest … skipped` line is printed and the walk continues. Given a usable manifest beside a same-spec `.js`, the `.js` is shadowed.
- **AC-10:** Given a comemory-style `.sh` pre-tool module that asks and a built-in gate that denies, the result is the built-in's deny (fixture case).
- **AC-11:** Given a Codex snapshot that lists plugin A but not B, `prune_inactive_modules` removes B's `.js`, `.sh` and `.json` regular files in both directories. It keeps A's, symlinks and un-namespaced files, and prunes nothing with a stale or missing snapshot or on another host.
- **AC-12:** Given real git repositories, `push_target_root` resolves a `-C "<dir with space>"` chain to that worktree's toplevel, and falls back to the cwd's toplevel for a dynamic `-C "$D"`. `push_target_branch` returns the checked-out branch, or on a detached HEAD the refspec destination (`HEAD:feat/y` gives `feat/y`, `:gone` gives `""`). `is_git_push` and `is_git_commit` map `unknown` to false.
- **AC-13:** `toolu hook pre-tools` with a payload and a registry `.sh` that exits 2 exits 2 with the module's stderr. `toolu hook post-tools` with a block module prints the block JSON and exits 0.
- **AC-14:** `cargo xtask gate` passes with `toolu-engine` at or above its 90% coverage floor, and `bun run test` passes apart from the documented environmental baseline.

## Acceptance evidence

| AC | Real input / fixture | Expected | Check |
|---|---|---|---|
| AC-1 | `fixtures/dispatch/cases.json` (about 40 cases, sandbox git repos, real bash and jq modules, real Bun for `.js`) | exact stdout, stderr and exit per case, in both implementations | `cargo test -p toolu-engine --test dispatch_fixture`; `bun test packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts` |
| AC-2 | fixture cases `pre: .sh exit 2 blocks…` and `…module env and stdin…` | exit 2, module stderr, env and stdin echoed byte for byte | as AC-1 |
| AC-3 | Rust tests with `sleep 5` and `trap '' TERM; …; exit 2` and a `kill -SEGV $$` module, timeout 1 s | the `exited 124` and `exited 139` lines, later module's advisory present | `cargo test -p toolu-engine --test executable` |
| AC-4 | fixture cases `shadowed .sh`, `un-namespaced file` | matches the TypeScript capture | as AC-1 |
| AC-5 | Rust test: counting `Rule`, manifests `Write`-only, `PATH=<sentinels>:$PATH` | counter 0, sentinel log absent, empty output | `cargo test -p toolu-engine --test rules`; `cargo test -p toolu-cli --test tool_hooks`; `cargo xtask measure` output in the PR body |
| AC-6 | committed `plugins/ts-quality/hooks/dist/post-tool-use.js` in two identical TypeScript-project sandboxes. The TypeScript side runs through `bun packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts`, which wraps `dispatchPostTool` (request in, result out) | identical stdout, stderr and exit (sandbox paths normalized) and the same gate entry | `cargo test -p toolu-engine --test quality_bridge` |
| AC-7 | `PATH` without bun, `HOME` without `.bun`, `TOOLU_BUN` unset | deny text, then one advisory across two calls | `cargo test -p toolu-engine --test bridge` |
| AC-8 | sentinel `bun` wrapper that logs and `exec`s the real Bun | one log line, ordered results, isolation | `cargo test -p toolu-engine --test bridge` |
| AC-9 | test rules and manifests in a temp registry | as stated | `cargo test -p toolu-engine --test rules` |
| AC-10 | fixture case `pre: a comemory .sh ask loses to a built-in deny` | deny | as AC-1 |
| AC-11 | Codex home with a ready snapshot, symlink and un-namespaced files | removed list as stated | `cargo test -p toolu-engine --test prune` |
| AC-12 | real `git init`, `git worktree add --detach "<dir with space>"` | as stated | `cargo test -p toolu-engine --test detect` |
| AC-13 | `assert_cmd` on the built `toolu` | as stated | `cargo test -p toolu-cli --test tool_hooks` |
| AC-14 | whole workspace | green | `cargo xtask gate`; `bun run test` |

## Documentation impact

- `docs/registry.md`: a "Native engine" section covering the walk, manifests and matchers, executable deadlines and the `exited 124` line, the Bun bridge and its no-Bun behaviour, and manifest shadowing.
- `docs/detect.md`: command detection now lives in `toolu_engine::detect`.
- `docs/resource-budgets.md` is unchanged.
- `AGENTS.md`: a Key files row for `crates/core/engine/src/lib.rs`, and the `crates/cli/src/main.rs` row now names `pre-tools` and `post-tools` as native.
- `fixtures/README.md` and `fixtures/index.json`: the new `dispatch-cases` suite.
- `crates/core/engine` gets no README, since none is required.
- CHANGELOG is written by release-please.

## Open Questions

None blocking:
- The 30 s module deadline and the 8 MiB output budget are defaults chosen here. #425 sets the hook timeouts in `hooks.json` and can tune them, and the owner is #425.
- Whether a manifest's rule should also run when Bun-era `.js` modules of the same spec remain is settled here (the manifest shadows them). #426–#429 confirm it when they write the first manifests.
