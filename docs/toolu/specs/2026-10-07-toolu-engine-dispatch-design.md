# toolu-engine: registry runner and dispatch — Design

**Date:** 2026-10-07   **Status:** Approved (revision 4; spec review rounds 1–4)   **Author:** epic worker (#418)   **Topic:** port `@toolu/core/dispatch` and `@toolu/core/registry` to `crates/core/engine`, with compiled-in rules, executable modules and a temporary Bun bridge

## Problem

Every tool call runs toolu's PreToolUse and PostToolUse dispatchers. Today they are Bun bundles that cost 48–56 MiB and 91–166 ms CPU per spawn on Linux CI (`docs/resource-budgets.md`). Epic #402 moves them into the `toolu` binary. The gates (#419–#423) and the rule crates (#426–#429) all plug into one walk: built-ins in table order, then the registry. That walk does not exist in Rust yet. Until it does, no gate can be ported against the real ordering, precedence, per-path patch handling and third-party module contract that users and plugins depend on.

## Non-Goals

1. No built-in gate is ported. The built-in tables are empty, and #419–#423 fill them.
2. `hooks.json` keeps running the Bun bundles, and `fixtures/rust-ported.json` stays empty. No host reaches the native `toolu hook pre-tools` or `post-tools` before #425. So `bun run bench:hooks --assert`, which gates only ported entries, cannot measure the native entries yet. The issue's "checked by the benchmark" is met by a runnable `cargo xtask measure` check instead (AC-5).
3. The TypeScript dispatcher and registry do not change. TypeScript gets only test-side files: the shared-golden harness and test, and `dispatch-cli.ts`, a request-in, result-out wrapper the live A/B test drives.
4. No new module kind is admitted. The registry reads `.js`, `.sh` and `.json` (`toolu_runtime::registry::parse_name`). Arbitrary executables and #440's executable protocol are out of scope.
5. The manifest file format of #414 does not change. No plugin writes manifests yet; that starts with #426.
6. The `mcp__` hook (`mcp-tools`) and agent-tier are not wired; #422 owns them.
7. No resource budget changes.

## Architecture

The walk ports these TypeScript symbols:

- `dispatchHook`, `sessionFor`, `dispatchInput`, `dispatchRecords` and `syntheticEdit` (`dispatch.ts`);
- `dispatchModules`, `walkBuiltins`, `walkRegistry`, `consume`, `settle`, `moduleEnv` and `encoded` (`dispatch-walk.ts`);
- `readField`, `collectAdvisories`, `finalAsk` and `finalAdvisory` (`dispatch-output.ts`);
- `runBash` and `moduleStdin` (`dispatch-bash.ts`);
- `toolEvent` and `toolContext` (`dispatch-context.ts`);
- `runRegistry` (`registry-run.ts`), `listRegistryDir` (`registry-list.ts`), `pluginPresence` (`registry-gate.ts`) and `pruneInactiveModules` (`registry-prune.ts`).

Module layout under `crates/core/engine/src/`:

| Module | Holds |
|---|---|
| `lib.rs` | crate doc, re-exports |
| `gate.rs` | the `Gate` trait (built-ins) |
| `builtins.rs` | `PRE_TOOL` and `POST_TOOL`, empty |
| `trace.rs` | `Step`, `StepKind`, `StepStatus`, `Skip` |
| `dispatch.rs` | `DispatchOptions`, `Dispatched`, `ModuleResult`, `dispatch_pre_tool`, `dispatch_post_tool` |
| `dispatch/session.rs` | payload parse, config switch, `Session` (env, roots, project root) |
| `dispatch/event.rs` | `NormalizedEvent`, `RuleContext` and the bridge's ordered event and ctx |
| `dispatch/edits.rs` | the edit-record split, the synthetic `Edit`, the per-path fold, `continue_post_blocks` |
| `dispatch/walk.rs` | built-ins, then the registry phase, then consumption |
| `dispatch/fold.rs` | `consume`, `settle` and the walk state |
| `dispatch/output.rs` | jq-compatible reading and printing |
| `registry.rs` | `Entry`, `Listing`, `list_dir` |
| `registry/gate.rs` | installed-plugin gating |
| `registry/prune.rs` | the Codex prune |
| `registry/manifests.rs` | manifest reading, rule lookup, matching, shadowing |
| `registry/phase.rs` | the registry phase: ordered execution and the stop rule |
| `registry/executable.rs` | `.sh` modules |
| `registry/bridge.rs` | batching and result parsing |
| `registry/bridge/runner.rs` | the embedded JavaScript runner and the request |
| `registry/bridge/bun.rs` | Bun resolution and the per-session marker |
| `detect.rs` | command detection |

Each module with a function gets `src/<dir>/tests/<module>_test.rs` (rule 7). Every file stays under 300 code lines.

The decisive trade-off is that consecutive `.js` modules share one Bun process (brainstorm, Jev 0.88). Order and stop-after-deny stay exact, and today's four quality modules cost one Bun start, not four. A timeout or crash isolates only the module in flight.

The CLI wiring is thin. `crates/toolu` (the hub, which owns rule crates) gets `tool_hook`, and `crates/cli/src/hook.rs` routes `toolu hook pre-tools` and `toolu hook post-tools` to it after the existing skew prelude.

`toolu-engine` dependencies:
- normal: `toolu-protocol`, `toolu-runtime`, `toolu-shell`, `toolu-state`, `serde`, `serde_json`;
- dev: `tempfile`, `serde_json`.

Reused:
- runtime: `Env`, `Roots`, `config::load` and `config::read::enabled`, `host::detect`, `host::snapshot::{codex_plugin_installed, Installed}`, the `registry` types, `read_manifest`, `Rule`/`RuleContext`, `process::run`, `json::{jq_text, ordered::Ordered}` and `git::toplevel`;
- protocol: `Decision`, `encode::encode`, `hook::{run_hook_io, Io, Reply, Raw}` and `NormalizedEvent`;
- state: `edit_records::normalize_edit_records`, `git::current_branch` and `js_order`;
- shell: `git::{runs_git_subcommand, push_targets}`.

## Interfaces / Schema

### Rust (`toolu-engine`)

```rust
/// A built-in gate (`ToolModule`): run in process before the registry.
pub trait Gate: Sync {
  fn name(&self) -> &str;
  /// `Err(_)` is a thrown error: the module counts as exit 1, so only the line
  /// `toolu-dispatch: module <name> exited 1; output skipped` is printed. The message is dropped, as in TypeScript.
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String>;
}

pub mod builtins { pub const PRE_TOOL: &[&dyn Gate] = &[]; pub const POST_TOOL: &[&dyn Gate] = &[]; }

pub struct DispatchOptions<'a> {
  pub env: &'a Env,                        // the hook's environment (HostEnv)
  pub cwd: &'a Path,                       // the hook process's working directory
  pub lib_dir: &'a Path,                   // TOOLU_LIB_DIR for .sh modules
  pub builtins: &'a [&'a dyn Gate],
  pub rules: &'a [&'a dyn Rule],           // compiled-in rules a manifest may enable
  pub selected_specs: Option<&'a BTreeSet<String>>,  // selectedRegistrySpecs (OpenCode)
  pub continue_post_blocks: bool,          // continuePostBlocks (OpenCode)
  pub module_timeout: Duration,            // per .sh module and per .js module in a batch; DEFAULT_MODULE_TIMEOUT = 30 s
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
pub fn plugin_presence(spec: &str, roots: &Roots) -> Installed;   // toolu_runtime::host::snapshot::Installed
pub fn plugin_active(spec: &str, roots: &Roots) -> bool;
pub fn prune_inactive_modules(roots: &Roots) -> Vec<PathBuf>;     // caller: #424's SessionStart housekeeping

// detect
pub fn is_git_push(analysis: &ShellAnalysis) -> bool;
pub fn is_git_commit(analysis: &ShellAnalysis) -> bool;
pub fn push_target_root(analysis: &ShellAnalysis, roots: &Roots, cwd: &Path) -> PathBuf;
pub fn push_target_branch(analysis: &ShellAnalysis, root: &Path, env: &Env) -> String;
```

Additive changes to lower crates:
- `toolu_runtime::process::Spec::wait: Wait`. `Group` keeps today's behaviour and stays the default. `Streams` returns once the child has exited and both its streams are closed, leaving anything it backgrounded alone, as `Bun.spawnSync` does. The deadline still terminates the whole group.
- `toolu_runtime::invocation::current_dir() -> PathBuf` returns the process working directory, or `.`.
- `toolu_runtime::registry::rule::Rule` gains a provided method, `fn applies(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> bool { true }`. The engine calls `run` only when the manifest matcher fits and `applies` is true (Jev 0.68 over recording a deviation). #426 implements it by file extension.
- `toolu_state::js_order` becomes public and gains `pub fn js_ordered(value: Ordered) -> Ordered`. It reorders every object at every depth the way `JSON.parse` orders keys: canonical array indices first, ascending, then insertion order.

### The walk (byte-exact with TypeScript)

**Config** comes first, as in `dispatchHook`. `hooks.<pre|post>-tools: false` makes the hook print nothing, whatever the payload. Config warnings come first on stderr, as `toolu-config: <line>`.

**Stdin.** Every trailing newline is stripped (`substituted`). That text is the payload: it is `input`, and with one `\n` added it is the module's stdin. It is parsed for reading only (`parseDocument`), in document order, with keys reordered as `JSON.parse` orders them:
- Escapes of lone UTF-16 surrogates inside JSON strings are rewritten to the escape `\uFFFD` in the parsed copy only, because `JSON.parse` accepts them and serde does not. That covers a high surrogate not followed by a low-surrogate escape, and a low surrogate not preceded by a high one. A valid pair and a literal backslash-u sequence (`\\ud800`) stay as they are. Gates, rules and the Bun bridge's `event` and `raw` see U+FFFD there. Modules still get the raw text.
- A payload that serde rejects with its recursion limit (an error whose message starts with `recursion limit exceeded`, about 128 levels) fails closed. Before a tool it prints a deny, `toolu: the hook payload nests too deeply to be checked`; after a tool, a block with the same reason (Jev 0.72). Both are fixed compact lines like `MALFORMED_PATCH_DENY` and `MALFORMED_PATCH_BLOCK`: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"<reason>"}}\n` and `{"decision":"block","reason":"<reason>"}\n`, each with exit 0. A test pins the deepest payload that still walks and the first one that fails closed, so a serde change cannot silently move the boundary.
- Any other parse failure is "not JSON", as in TypeScript. The tool name reads as `""`, so it is not an edit, and the raw text walks once.

**Session.**
- Before a tool: the environment gains `TOOLU_CONFIG_DIR`, and the project root is `Roots::project_root(cwd)`, else the cwd.
- After a tool: `PROJECT_ROOT` is the git toplevel of the cwd, else the cwd. `PATH` becomes `<PROJECT_ROOT>/node_modules/.bin:<PATH or "">`, and `TOOLU_CONFIG_DIR` is added.
- The host is `host::detect(env, None)`. Encoding uses Codex for Codex and Claude for every other host (`encoded`).

**Event** (`toolEvent`): the session id is `session_id` or `"unknown"`, and `cwd` is `cwd` or the project root. `projectRoot` and `worktree` are the session's project root. `toolCallId` is `tool_use_id` or `"unknown"`, and `toolName` is the walk's tool name or `"unknown"`. `toolInput` is `tool_input` when it is an object, else `{}`. A non-string or empty value counts as absent. The type:
- after a tool, `tool/post`, with `toolOutput` set to `tool_response ?? tool_output` when that is not undefined;
- before a tool, `shell/pre`, with `command`, for `Bash` or `Shell` and a non-empty string command;
- otherwise `tool/pre`.

The `RuleContext` is the host, the session environment, the config root, the project root, the cwd, `raw` (the payload object, else `{}`) and, for a split path, `edit`.

**Edits** (`dispatchInput`, `dispatchRecords`): an edit tool is split with `normalize_edit_records`.
- `Malformed`, empty records, or a payload that is not an object print the fixed `MALFORMED_PATCH_DENY` or `MALFORMED_PATCH_BLOCK`.
- Each record walks the synthetic payload: `jq -c` of the payload with `tool_name` set to `"Edit"`, and `tool_input` extended (in place where keys exist) with `file_path` and `path` set to the record path and with `toolu_edit_operation`, `toolu_edit_from` and `toolu_edit_moved_to`. Keys follow `JSON.parse` order. `TOOLU_EDIT_OPERATION` is the record's operation, `TOOLU_EDIT_FROM` is `record.from ?? ""` and `TOOLU_EDIT_MOVED_TO` is `record.moved_to ?? ""`. The bridge `edit` and `RuleContext::edit` (`EditSplit`) carry the same three values; the `EditSplit` doc in `rule.rs` is corrected to say so.
- **Two-level fold.** Each path's walk settles on its own. Its stderr is appended to the outer stderr, and its stdout, with trailing newlines stripped, is consumed by an outer walk state as if it were a module. So an ask held from one path is enriched again with the advisories of the other paths, and registry warnings repeat once per path.
- A deny, block or exit 2 on any path ends the whole patch.
- With `continue_post_blocks` after a tool, a path whose stdout is a block adds its `reason` (or `check blocked`) and the walk goes on. That path's other output is dropped. A non-zero path exit returns at once. At the end, any collected reasons print as `{"decision":"block","reason":"<r1>\n\n<r2>"}`.

**One walk** (`dispatchModules`):
1. **Built-ins** run in table order and are consumed one at a time.
2. **The registry phase** (`runRegistry`) lists `<config root>/toolu/<pre-tools.d|post-tools.d>`:
   - Entries come in byte order of their names. Dotfiles and anything that is not a file after following symlinks are ignored. Un-namespaced module files each add `toolu-registry: registry module <file> lacks <plugin-spec>__<name> namespace; skipped` to stderr at once, and never run.
   - Each entry is then judged and, if it should, executed, in order:
     - **Gating.** The entry is `Skipped(Inactive)` when its plugin is definitively absent: missing from Claude's `installed_plugins.json` (`CLAUDE_PLUGINS_REGISTRY`, else `<TOOLU_CONFIG_DIR | CLAUDE_CONFIG_DIR | $HOME/.claude>/plugins/installed_plugins.json`), or from a ready Codex snapshot. It is also skipped when `selected_specs` is set and lacks its spec. An unreadable record fails open, and other hosts count as installed. Answers are memoized per spec per walk.
     - **Shadowing.** A `.sh` entry is `Skipped(Shadowed)` when any `.js` entry in the directory has its spec. A spec with at least one usable manifest shadows its `.js` and `.sh` entries.
     - **Manifests**, in this order:
       1. `read_manifest`. A problem adds `toolu-registry: manifest <file> skipped: <reason>` at once and is never an error. The reason is `read_manifest`'s message without its path prefix. Problems are: unreadable, an unknown field, a `version` other than 1, a spec, name or event that does not match the file, or an empty matcher entry.
       2. The matcher, against the walk's tool name. That name is `"Edit"` for every split edit and `""` for stdin that is not JSON. A miss is a silent `Skipped(NotMatching)`.
       3. The compiled-in rule of that spec, name and event. None adds `toolu-registry: manifest <file> skipped: no rule <spec>__<name> in toolu <VERSION>`, so a plugin newer than the binary is reported only when its rule would apply.
       4. `applies`. False is `Skipped(NotMatching)`.
       5. Otherwise `Rule::run` decides.

       A manifest is *usable*, for shadowing, when step 1 passes and its rule exists, whatever the tool. Every manifest in the directory is read once, when the registry phase starts. Its problem line is printed only when the walk reaches it, after gating, so a manifest of an inactive or unselected plugin is silent. Lines appear once per walk, and so once per patch path.
     - **`.sh`.** `bash <path>` runs with the cwd, stdin is the payload plus `\n`, and the environment is the session's plus `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and `TOOLU_EDIT_*`. The wait is `Streams`. The status is the exit code, or 128 plus the signal number. Every trailing newline is stripped from stdout.
       - A spawn failure, such as `bash` missing or an environment over the kernel's limit (E2BIG, for instance a large Write `input`), is status 127.
       - Past `module_timeout` the group is killed and the status is fixed at **124**, so a timed-out module can never deny.
       - Output beyond 8 MiB (stdout and stderr together) truncates. An exit 2 still denies, with its stderr as far as it was kept. Any other truncated module is skipped, and consumption prints `toolu-dispatch: module <file> printed more than 8388608 bytes; output skipped` where an `exited` line would go.
     - **`.js`.** Run through the Bun bridge (below). A failure adds `toolu-registry: module <file> failed: <error>; output skipped` at once.
   - The phase stops after the entry whose result stops the walk: before a tool, a `.sh` exit 2, or a `.sh`, rule or `.js` that denies; after a tool, a `.sh` exit 2, or one that blocks. Entries after a wrong-kind decision (`runtime_failure`, or `post_block` before a tool) still run, as in TypeScript.
3. **Consumption** (`consume`), over built-in results as they come and registry results after the phase, in order:
   - Exit 2 ends the walk: stdout empty, stderr is the walk's stderr plus the module's, exit 2.
   - Any other non-zero status adds `toolu-dispatch: module <file|name> exited <status>; output skipped`. The module's stderr is dropped here, and also on exit 0.
   - Before a tool, the first `permissionDecision: "deny"` prints verbatim plus `\n` and ends the walk. The first `ask` is held.
   - After a tool, the first `decision: "block"` does the same, and `permissionDecision` is ignored.
   - Otherwise `additionalContext` and `systemMessage` are collected (read like `jq -r '… // empty'`) and deduplicated by exact text.
   - Built-in, rule and `.js` decisions are first encoded with `toolu_protocol::encode::encode`.

   The resulting stderr order is: config warnings, built-in `exited` lines, then registry `toolu-registry:` lines in walk order, then registry `exited` lines in consumption order.
4. **Settle.** A held ask gets the joined contexts appended to `permissionDecisionReason` and the joined messages to `systemMessage`, each after a blank line. It is printed like `jq -n`, or as written plus `\n` when that enrichment would fail in jq, for example a non-string reason. Without an ask, the merged advisory prints like `jq -n` with `hookEventName` set to `PreToolUse` or `PostToolUse`. Otherwise stdout is empty.

### Bun bridge protocol

- **Bun** is resolved from `TOOLU_BUN` (a regular executable file), then `bun` on the session `PATH`, then `$HOME/.bun/bin/bun`.
- **A batch** is a maximal run of consecutive `.js` entries that will execute. Gated, shadowed and rejected entries do not split a batch. An executing `.sh` or rule does.
- **Spawn.** The batch is spawned as `[bun, "--no-install", "-e", RUNNER]`, with the session environment and the hook's cwd. Its deadline is `module_timeout × modules in the batch` (Jev 0.93), and the wait is `Streams`, so a process a module backgrounds does not hold the batch. A module that hangs first in a batch of four holds the others for the whole scaled deadline before they re-run; `docs/registry.md` says so, and the hook timeout #425 sets bounds it.
- **Request** on stdin (keys in this order; `event` and `ctx` keys follow `toolEvent` and `toolContext`):
  ```json
  {"stop":"deny"|"post_block","registryEvent":"tool/pre"|"tool/post",
   "event":{"sessionId","cwd","projectRoot","worktree","toolCallId","toolName","toolInput","type","command"?|"toolOutput"?},
   "ctx":{"host","configRoot","projectRoot","cwd","raw","edit"?:{"operation","from","movedTo"}},
   "modules":[{"file","path","spec","name"}]}
  ```
- **Runner.** It sets `ctx.env = process.env`. For each module it imports the path and checks the default export as `runContract` does: an object (not an array) whose `spec` and `name` are strings, whose `event` is `tool/pre` or `tool/post`, and whose `run` is a function. The export must match the file and directory. It then calls `run.call(exported, event, ctx)` and validates the result as `DecisionSchema` does, keeping only schema fields. Each module gets one line, written with `fs.writeSync(1, …)`: `\ntoolu-bridge:{"file":…,"decision":{…}}` or `\ntoolu-bridge:{"file":…,"error":"…"}`. The error messages are TypeScript's:
  - `default export is not a registry module`;
  - `contract mismatch: exports {…}, file and directory want {…}`;
  - `invalid decision: <util.inspect(result)>`;
  - the thrown message.

  The runner stops after a decision whose kind is `stop`.
- **Results.** The engine reads only the lines that start with the marker. A decision is parsed strictly. Other stdout is dropped. Bun's stderr is appended to the walk's stderr when the batch ends.
- **Isolation.** When a batch ends before every module has a line, the first module without one fails with:
  - `bridge exited <status>`;
  - `timed out after <deadline ms> ms`;
  - `bridge output unreadable` (for example a malformed or out-of-order line).

  The modules after it run in a fresh batch.
- **No Bun, or Bun cannot be spawned:**
  - Before a tool, the first applicable `.js` module is a deny: `toolu-registry: module <file> needs Bun 1.4.x, which was not found (checked TOOLU_BUN, PATH and ~/.bun/bin/bun); install it from https://bun.sh or remove the module`. Its step is `Skipped(NoBun)`, and the walk stops.
  - After a tool, every `.js` module is `Skipped(NoBun)`. The walk adds one advisory decision in the place of the first such module, consumed in that position like any module's and encoded the same way (`additionalContext` on Claude and Codex): `toolu-registry: <n> registry module(s) did not run because Bun was not found: <files>. Install Bun 1.4.x from https://bun.sh.`
- **Per-session marker.** The post advisory appears once per session:
  - The marker is `<project_state_dir("registry-bridge", root = the session's project root)>/<session>`. After a tool, that root is `PROJECT_ROOT`, the git toplevel or the cwd. `<session>` is the payload's `session_id` with every character outside `[A-Za-z0-9._-]` replaced by `_`, cut to 128 characters, and `unknown` when empty or absent.
  - The marker is created with `create_new`. If it already exists (`AlreadyExists`, including a concurrent hook that won the race), the advisory is suppressed. Any other failure to create the directory or file still shows the advisory, so it repeats rather than disappears.
  - Markers are empty files and are not swept. They exist only while Bun is missing and `.js` modules remain.

### Shared fixture: `fixtures/dispatch/cases.json`

```json
{ "version": 1, "cases": [ {
  "name": "…", "phase": "pre"|"post", "host": "claude"|"codex",
  "stdin": <object, sent as JSON.stringify(object)> | "<raw text, sent as is>",
  "config": <toolu.config.json object>?,            // $PROJECT/<.claude | .codex>/toolu.config.json
  "builtins": [ {"name": "…", "decision": <Decision>} | {"name": "…", "throws": "<message>"} ]?,
  "registry": [ {"file": "<name>", "sh": "<bash body>"} | {"file": "<name>", "js": "<ESM source>"} ]?,
  "installed": ["<spec>", …]?,       // Claude installed_plugins.json, or a ready Codex snapshot
  "installedRaw": "<text>"?,         // a raw installed_plugins.json, for fail-open cases
  "continuePostBlocks": true?,
  "expect": {"stdout": "…", "stderr": "…", "exitCode": 0|2}
} ] }
```

- **Sandbox.** Each case gets a git `project`, a `home`, a Codex home and a plugin root. The cwd is `$PROJECT`.
- **Environment.** `PATH` is the test's own, and `HOME` is set. Claude adds `CLAUDE_PROJECT_DIR=$PROJECT`. Codex adds `PLUGIN_ROOT=<root>` and `CODEX_HOME=$CODEX`.
- **Files.** `.sh` files get `#!/usr/bin/env bash` and mode 0755.
- **Tokens.** `$PROJECT`, `$HOME`, `$CODEX`, `$CONFIG` (the host's config root) and `$LIB` (`<root>/plugin/hooks/lib`) expand in `stdin` strings, module bodies and `expect`.
- **Capture.** `expect` is captured once from the TypeScript dispatcher and committed. `packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts` (TypeScript) and `crates/core/engine/tests/dispatch_fixture.rs` (Rust) must both reproduce it byte for byte.
- **Index.** The suite is listed in `fixtures/index.json` (`dispatch-cases`) and `fixtures/README.md`.

### CLI

- **Entry.** `toolu_hub::tool_hook(phase: toolu_engine::Phase, payload: std::io::Result<String>, plugin_root: Option<&Path>) -> Outcome`, where `pub enum Phase { Pre, Post }` lives in `toolu_engine::dispatch` and is re-exported as `toolu_engine::Phase`. The payload is what `Context::stdin` returned. An `Err` reaches `run_hook_io` as a reader that fails with it, so the protocol reports it exactly as an unreadable payload. It reads `Env::process()` and `invocation::current_dir()`. It takes `TOOLU_LIB_DIR` from `<plugin root>/hooks/lib`, where the plugin root is `--plugin-root`, else `Roots::plugin_root()`, else the empty path.
- **Run.** It passes `builtins::{PRE_TOOL|POST_TOOL}`, the hub's `RULES` (empty), `selected_specs: None`, `continue_post_blocks: false` and `DEFAULT_MODULE_TIMEOUT`. It runs the engine inside `run_hook_io` over in-memory streams, with `HostEvent::ToolPre` or `ToolPost` and the detected host. A panic therefore exits 2 with `blocked: toolu PreToolUse hook panicked: …`, or after a tool the same line without `blocked: `.
- **Output.** Stdout and stderr each lose exactly one trailing `\n` and become `None` when empty, because `output::emit` writes each with `writeln!`. Engine stdout always ends in `\n` or is empty, so stdout is byte-exact. Stderr is byte-exact when it ends in `\n`. Otherwise the CLI adds one, and a stderr of exactly `\n` prints nothing. This is a documented deviation; the engine's own result stays exact. The exit is `Blocked` when `run_hook_io` returns 2, else `Success`.
- **Routing.** `crates/cli/src/hook.rs` maps `toolu` plugin hooks named `pre-tools` and `post-tools` to it after the skew prelude. `--event` is ignored, since the name fixes the event.
- **Payload.** The CLI reads it with `Context::stdin`. A payload that is not UTF-8 fails closed, as `run_hook_io` reports an unreadable payload: `blocked: …` and exit 2 before a tool, the same line and exit 2 after it. Bun decoded such bytes lossily; this is a documented deviation.

## Failure modes and edge cases

| Input | Behaviour | Propagation |
|---|---|---|
| Stdin with trailing newlines | stripped before anything | recovered, parity |
| Stdin not JSON | tool name `""`, not an edit; the raw text walks once | recovered, parity |
| A lone surrogate escape in the payload | parsed with U+FFFD in the reading copy; gates see the tool | recovered; the synthetic `Edit` re-prints U+FFFD where TypeScript prints `\ud800` (documented) |
| Payload nested deeper than 128 | deny before a tool, block after it | fail closed |
| Absent registry directory | no entries, nothing spawned | recovered |
| Install record unreadable, malformed or absent; Codex snapshot missing or stale | plugin counts as installed | fail open, parity |
| Registry file removed or rewritten mid-walk (concurrent SessionStart) | listing is taken once; a vanished manifest is a manifest line; a vanished `.sh` exits 127 and is skipped | converted |
| `.sh` exit 2 | hard deny: exit 2 with the module's stderr | propagates |
| `.sh` exit 1 or 3, or exit 0 with stderr | `exited <n>` line; the module's stderr is dropped | converted, parity |
| `.sh` killed by a signal | `exited <128+n>; output skipped`; the walk goes on | converted |
| `.sh` past the deadline, including one that traps SIGTERM and exits 2 | group killed, `exited 124`; the walk goes on | converted |
| `.sh` backgrounds a process with its streams redirected | not waited for | recovered, spawnSync parity |
| `.sh` over 8 MiB of output | `printed more than 8388608 bytes; output skipped` | converted |
| `bash` missing, or E2BIG for a huge `input` | status 127, skipped | converted, parity |
| `.js` import error, throw, contract mismatch, invalid decision | `toolu-registry: module <file> failed: …` | converted, parity |
| `.js` module calls `process.exit` or hangs | that module fails; the rest re-run in a new batch | converted |
| A patch of N paths with `.js` modules | one Bun batch per path per run of consecutive `.js` | documented cost |
| Bun missing before a tool, with an applicable `.js` | deny naming the module | fail closed |
| Bun missing after a tool | one advisory per session; the modules are skipped | converted |
| Manifest problems or an unknown rule | one `manifest … skipped` line | converted |
| Rule or gate panics | `run_hook_io`: `blocked: toolu PreToolUse hook panicked: …`, exit 2 (post: no `blocked: `, exit 2) | fail closed |
| `apply_patch` headers unparseable; edit without a path | fixed malformed deny or block | fail closed, parity |
| Payload not UTF-8 (CLI) | `run_hook_io` unreadable-payload failure, exit 2 | fail closed; Bun decoded lossily (documented) |
| Module stderr not ending in `\n`, at the CLI | the CLI adds a final newline; a lone `\n` prints nothing | documented deviation; the engine's output is exact |
| `.sh` exit 2 with over 8 MiB of output | still a deny; stderr cut at the budget | fail closed |

## Acceptance criteria

- **AC-1:** Given each case in `fixtures/dispatch/cases.json`, the Rust engine produces the captured stdout, stderr and exit code byte for byte, and so does the TypeScript dispatcher. The cases cover:
  - precedence: deny over ask over advisory, block over advisory;
  - module ordering in byte order, and two-phase stderr ordering;
  - the per-path fold and the malformed-patch replies;
  - `continuePostBlocks`, gating, shadowing and the namespace rejection;
  - the Codex degradation of a built-in ask, and the config switch;
  - raw and trailing-newline stdin.
- **AC-2:** Given a third-party `.sh` module that reads stdin and its environment and exits 2, the tool is blocked (exit 2) with only the module's stderr appended. The module received the payload plus `\n` on stdin, and `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and `TOOLU_EDIT_*` exactly as TypeScript gives them. Exit 3 and exit 0 with stderr are skipped or ignored with their stderr dropped.
- **AC-3:** Given a `.sh` module that sleeps past the deadline, one that traps TERM and then exits 2, or one killed by SIGSEGV, the walk reports `toolu-dispatch: module <file> exited 124|139; output skipped`, never denies, and the modules after it still run.
- **AC-4:** Given `x@t__old.sh` and `x@t__new.js` of the same spec, only the `.js` runs, and the trace shows `x@t__old.sh` as `Skipped(Shadowed)`. Given `noname.sh`, it never runs and stderr carries the namespace line. The output matches TypeScript's fixture capture.
- **AC-5:** Given a `Read` call and a registry of only manifests whose matchers do not fit `Read`:
  - no rule's `applies` or `run` is called;
  - the trace holds only `Skipped(NotMatching)` steps;
  - sentinel `bash` and `bun` executables first on `PATH` are never run.

  The release `toolu hook pre-tools`, measured by `cargo xtask measure` with that payload and registry, stays within the pre-tools RSS budget of 6 MiB and leaves the sentinel log empty.
- **AC-6:** Given the committed ts-quality bundle `plugins/ts-quality/hooks/dist/post-tool-use.js`, registered as `ts-quality@toolu__ts-quality.js`, in a TypeScript project (tracked `tsconfig.json`, `bun.lock`, Bun on `PATH`), and an Edit of a `.ts` file with a violation:
  - the Rust post-tool dispatch prints the same stdout, stderr and exit code as the TypeScript dispatcher run live on an identical sandbox;
  - both write the same gate-file entry, compared with `updatedAt` and sandbox paths normalized.
- **AC-7:** Given Bun absent (`TOOLU_BUN` unset, no `bun` on `PATH`, no `$HOME/.bun/bin/bun`) and an active pre-tool `.js` module, the pre-tool dispatch denies, naming the module. Given the same after a tool, the first call with `session_id: s1` prints the advisory, and a second call with `s1` prints none. A missing `session_id` uses `unknown`.
- **AC-8:**
  - Three consecutive `.js` modules run in one Bun process: a sentinel `bun` wrapper logs one spawn, results come in byte order, and the walk stops after a deny.
  - A `.sh` between two `.js` modules makes two spawns.
  - A middle module that hangs or calls `process.exit(3)` fails alone (`timed out after … ms` or `bridge exited 3`), and the third module still runs.
- **AC-9:**
  - Given a usable manifest `ts-quality@toolu__ts-quality.json` (matcher `Edit`) for a test rule whose `applies` holds only for `.ts` files, a `.py` edit leaves the rule unrun (`Skipped(NotMatching)`), while a `.ts` edit runs it.
  - A `Write` or `apply_patch` call reaches the rule as `Edit`; a manifest with matcher `Write` never fires.
  - `*` and `mcp__*` matchers fit as defined.
  - A manifest with `version: 2` or an unknown field prints one `toolu-registry: manifest … skipped: …` line, whatever the tool. A manifest whose matcher fits but whose rule is not compiled in prints one `… skipped: no rule …` line, and one whose matcher does not fit prints none. The walk continues either way.
  - A usable manifest beside a same-spec `.js` shadows the `.js`.
- **AC-10:** Given a comemory-style `.sh` pre-tool module that asks and a built-in gate that denies, the result is the built-in's deny (fixture case).
- **AC-11:** Given a Codex snapshot listing plugin A and not B, `prune_inactive_modules` removes B's regular `.js`, `.sh` and `.json` files in both directories. It keeps A's, symlinks and un-namespaced files. It prunes nothing when the snapshot is stale or missing, or when the host is not Codex.
- **AC-12:** Given real git repositories:
  - `push_target_root` resolves `-C "<dir with space>"` to that worktree's toplevel. It falls back to the cwd's toplevel for a dynamic `-C "$D"` and for a missing directory, and to the cwd outside any repository.
  - `push_target_branch` returns the checked-out branch. On a detached or unborn HEAD, which `git rev-parse --abbrev-ref HEAD` cannot name, it returns the refspec destination (`HEAD:feat/y` gives `feat/y`, `:gone` gives `""`).
  - `is_git_push` and `is_git_commit` map `unknown` to false.
- **AC-13:**
  - `toolu hook pre-tools` given a payload and a registry `.sh` that exits 2 with newline-terminated stderr exits 2, and stderr is exactly the module's.
  - `toolu hook post-tools` given a block module prints exactly the block JSON plus one newline and exits 0.
  - A `Read` with nothing registered prints nothing and exits 0.
- **AC-14:** `cargo xtask gate` passes, including `toolu-engine` at or above its 90% coverage floor, and `bun run test` passes. The checks are the `rust` and `ts` CI jobs. The plan records how this root dev host reproduces them.
- **AC-15:** Given payloads of real hook shape:
  - a Bash call whose command holds a lone `\ud800` escape still reaches the gates as `shell/pre`;
  - a valid surrogate pair, a high surrogate followed by a non-low escape, a lone low surrogate and a literal `\\ud800` are read as stated;
  - the deepest nesting serde parses walks normally, and one level deeper denies before a tool and blocks after it;
  - with `hooks.pre-tools: false`, that too-deep payload prints nothing.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary / failure case | Check |
|---|---|---|---|---|
| AC-1 | `fixtures/dispatch/cases.json` (~50 cases: git sandboxes, real bash and jq `.sh`, real ESM `.js`, Bash, Write, Edit and apply_patch payloads, raw stdin) | exact stdout, stderr and exit per case | invalid JSON, trailing newlines, malformed patch, disabled hook, two-phase stderr | `cargo test -p toolu-engine --test dispatch_fixture`; `bun test packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts` |
| AC-2 | fixture cases `.sh exit 2…`, `…payload on stdin and the documented environment`, `…Write…`, `…apply_patch…` | exit 2 with module stderr; environment and stdin echoed byte for byte | exit 3 and exit 0 with stderr dropped | as AC-1 |
| AC-3 | Rust test, `module_timeout` 1 s: `sleep 5`; `trap '' TERM; sleep 5; exit 2`; `kill -SEGV $$`; a backgrounded `sleep 30 >/dev/null 2>&1 &` that then prints an advisory | `exited 124`, `exited 139`, later module's advisory; the backgrounder's advisory is present and it has no `exited 124` line | TERM-trapping exit 2 is not a deny | `cargo test -p toolu-engine --test executable` |
| AC-4 | fixture cases `shadows`, `un-namespaced`; Rust trace test | matches capture; trace `Skipped(Shadowed)` | `.sh` of another spec still runs | as AC-1; `cargo test -p toolu-engine --test walk` |
| AC-5 | counting `Rule` (counts `applies` and `run`), manifests `Write`/`Edit`-only, `PATH=<sentinels>:$PATH`; release binary under `cargo xtask measure` | counter 0, sentinel log absent, empty output, max RSS ≤ 6 MiB | a `*` manifest does run (control) | `cargo test -p toolu-engine --test rules`; `cargo test -p toolu-cli --test tool_hooks`; the plan's `measure-read` check (see below) |
| AC-6 | committed `plugins/ts-quality/hooks/dist/post-tool-use.js` in two identical TS-project sandboxes; TypeScript via `bun packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts` | identical outputs and gate entry (normalized) | a clean `.ts` edit also matches | `cargo test -p toolu-engine --test quality_bridge` |
| AC-7 | `PATH` without bun, `HOME` without `.bun`, `TOOLU_BUN` unset | deny text; one advisory over two `s1` calls | missing `session_id`; unwritable marker dir (a file in its place) repeats the advisory | `cargo test -p toolu-engine --test bridge` |
| AC-8 | sentinel `bun` wrapper that logs and `exec`s the real Bun | one log line; ordered results; isolation messages | single `.js`; `.sh` splitting a batch | `cargo test -p toolu-engine --test bridge` |
| AC-9 | test rules and manifests in a temp registry; real Write, Edit and apply_patch payloads | as stated | `*`, `mcp__*`, version 2, unknown field, unknown rule | `cargo test -p toolu-engine --test rules` |
| AC-10 | fixture case `a comemory .sh ask loses to a built-in deny` | deny | — | as AC-1 |
| AC-11 | Codex home with ready, indeterminate and missing snapshots, a symlink, un-namespaced files, Claude host | removed set as stated | non-Codex host and stale snapshot prune nothing | `cargo test -p toolu-engine --test prune` |
| AC-12 | real `git init`, `git worktree add --detach "<dir with space>"`, `git checkout --detach`, an unborn repo, a non-repo dir | as stated | dynamic `-C`, missing `-C` dir, outside a repo | `cargo test -p toolu-engine --test detect` |
| AC-13 | `assert_cmd` on the built `toolu` with real stdin | byte-exact stdout and stderr, exit codes | `Read` with an empty registry | `cargo test -p toolu-cli --test tool_hooks` |
| AC-14 | whole workspace | green | — | `cargo xtask gate --base origin/main`; `bun run test` (CI `rust` and `ts` jobs) |
| AC-15 | Rust payload test with real Bash, Write and post Edit payloads | as stated | depth boundary pinned on both sides; disabled hook | `cargo test -p toolu-engine --test payload` |

The `measure-read` check is the runnable plan ledger step `measure-read` in `docs/toolu/plans/2026-10-07-toolu-engine-dispatch.md` (`bun plugins/toolu/hooks/dist/plan-ledger.js run <plan> --step measure-read`). It does the following:
1. Builds the release `toolu` and the release `xtask`. The release measurer keeps the RSS floor near 3 MiB, as `hook-measurer.ts` `buildMeasurer` does; the debug one alone reads about 6 MB.
2. Writes the manifest `x@t__r.json` (matcher `Write|Edit`) under `$HOME/.claude/toolu/pre-tools.d`, and sentinel `bash` and `bun` scripts that append to `$S/spawned`.
3. Runs `HOME=$S/home PATH=$S/bin:/usr/bin:/bin target/release/xtask measure --out $S/m.json -- target/release/toolu hook pre-tools < read.json`.
4. Asserts `test ! -e $S/spawned` and `jq -e '.exitCode == 0 and .maxRssBytes <= 6291456' $S/m.json`.

## Documentation impact

- `docs/registry.md`: a "Native engine (Rust)" section covering:
  - the walk and its two phases;
  - manifests, matchers and `applies`;
  - executable deadlines and the `exited 124` line;
  - the Bun bridge, batching and its no-Bun behaviour;
  - manifest shadowing;
  - the payload-parsing rules;
  - the bridge deadline, and how a hung module holds the rest of its batch.
- `docs/detect.md`: command detection now lives in `toolu_engine::detect`.
- `AGENTS.md`: a Key files row for `crates/core/engine/src/lib.rs`. The `crates/cli/src/main.rs` row now names `pre-tools` and `post-tools` as native.
- `fixtures/README.md` and `fixtures/index.json`: the new `dispatch-cases` suite.
- CHANGELOG: written by release-please.

## Open Questions

None blocking:
- The 30 s module deadline and the 8 MiB output budget are defaults chosen here. #425 sets the hook timeouts in `hooks.json` and owns any tuning.
- Manifest shadowing of a same-spec `.js` is settled here. #426, which writes the first manifest, owns confirming it.
