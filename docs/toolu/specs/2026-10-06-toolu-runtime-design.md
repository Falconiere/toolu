# toolu-runtime: host roots, config, settings, process, startup publish, registry types — Design

**Date:** 2026-10-06   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** #414. The `runtime` core crate gets what every hook and verb needs to run on a host, without a shell parser or TLS.

Brainstorm: `docs/toolu/brainstorms/2026-10-06-toolu-runtime.md`. Builds on #413 (`toolu-protocol`: `Host`, `HostEvent`, `Decision`, `NormalizedEvent`, `supports_ask`, the native event-name tables), #442 (`cli::Ctx`, `Outcome`) and #455 (the quality bar).

## Problem

Every later Rust port reads the environment, finds the host's roots, loads `toolu.config.json` and fails closed on an envelope it cannot understand, resolves gate modes and thresholds, spawns `git` or `codex` with a bound, publishes helpers at SessionStart and reports what it did. Today all of that exists only in TypeScript (`@toolu/core/{host,config,process,startup,registry}`). Without one Rust home, #415 to #424 would each re-port part of it and drift from the TypeScript bytes that the two implementations share during coexistence. The registry types also have to sit below both the registry runner (#418) and startup, which form a cycle in TypeScript.

## Non-Goals

1. No hook or verb is ported, and the `toolu` binary does not change. The first callers are #415 (state), #418 (registry runner) and #424 (session lifecycle).
2. No strict whole-document config schema. `TooluConfigSchema`'s one production consumer is the OpenCode selection reader (`tools/toolu-opencode/src/inventory/selection.ts`), which #462 ports together with it (Jev `defer` 0.96). The hook-time loader checks only the envelope, as TypeScript does.
3. No OpenCode status record (`startup/opencode-status.ts`, #462) and no Bun helpers (`publishBunCli`, `bunOnPath`, `bunAdvisory`): the binary never publishes a Bun CLI.
4. No registry runner, module discovery, Codex prune or `register` write; those are #418. This change adds only the manifest type, the file-name rules and the `Rule` trait.
5. No parent-signal guard (`parent-guard.ts`) and no cancellation token for subprocesses: the Rust runner is synchronous and kills the group on its own deadline; the resident epic engine (#434) owns long-lived children.
6. Stable-path publishing replaces only symlinks, as TypeScript does. The issue's "or the launcher script" clause has no format anywhere in the repository; #443 (`toolu` on PATH) owns it and may extend the owned set (Jev `symlink_only` 0.83).
7. No gate data change: `[workspace.lints]`, `clippy.toml`, `rules.json` and the jscpd limits stay as they are.
8. The CLI argument helpers port only what clap does not already do: the jq number grammar of `numberValue`. `flagValue`, `CliExit` and `runCli` are replaced by the clap tree and `Outcome` (#442).

## Architecture

**Layering and capabilities.** `toolu-runtime` depends on `toolu-protocol`, `serde`, `serde_json`, `clap` (already) and `nix` (new for this crate, workspace dependency, feature `signal` added to `resource`). `cargo tree -p toolu-runtime` keeps no `tree-sitter*`, `brush*`, `rustls` or `ureq`. Per `rules.json`, `std::env` is read only here, and `std::process::Command` only in the `process` module. The crate never writes stdout or stderr (rule 14): every warning is returned as data and the caller prints it.

**Environment.** `Env` is an explicit snapshot (`BTreeMap<String, String>`); `Env::process()` reads `std::env::vars_os` lossily, `Env::from_pairs` builds one for tests and for a host that passes its own map. `get` treats an empty value as unset, like bash `${VAR:-}` and TypeScript `envValue`. A child process gets exactly the snapshot (`env_clear` then `envs`), which is TypeScript's `childEnv(env)`. `home()` is `HOME`, else `std::env::home_dir()`.

**Host.** Ports of `host-detect.ts`, `host-roots.ts` and `host-snapshot.ts`:
- `detect(env, hook_event_name)` resolves `TOOLU_HOST_OVERRIDE`, then a native event name only one host uses (`toolu_protocol::native::hosts_for_native`), then `CURSOR_VERSION`/`CURSOR_PROJECT_DIR`, then `PLUGIN_ROOT` (Codex), else Claude. An invalid override yields the warning `toolu-host: invalid TOOLU_HOST_OVERRIDE '<value>' (using environment detection)` in the result; detection continues.
- `Roots` binds an `Env` and a host (given, or detected once, keeping its one warning) and answers every path function: `config_root` (`TOOLU_CONFIG_DIR`, else `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `~/.cursor`, `HERMES_HOME`, `TOOLU_OPENCODE_HOME` or `XDG_CONFIG_HOME/opencode`), `project_root` (`TOOLU_PROJECT_DIR`, the host's own project variable — `CLAUDE_PROJECT_DIR` on Claude, `CURSOR_PROJECT_DIR` on Cursor — then `git rev-parse --show-toplevel` from `cwd` with the snapshot as the whole environment), `project_dirname` (`TOOLU_PROJECT_CONFIG_DIRNAME`, else `.<host>`), `project_config_path`, `project_state_root`, `project_state_dir`, `plugin_root` (`PLUGIN_ROOT`, `CURSOR_PLUGIN_ROOT` or `TOOLU_PLUGIN_ROOT` by host, then `CLAUDE_PLUGIN_ROOT`), `plugin_data` (`PLUGIN_DATA` on Codex, then `CLAUDE_PLUGIN_DATA`), `invocation`, `plugin_install_command` and `plugin_uninstall_command`.
- The Codex plugin snapshot: `codex_plugin_snapshot_path` (`TOOLU_CODEX_PLUGIN_SNAPSHOT`, else `<config root>/toolu/codex-plugins.json`), `snapshot_codex_plugins` (runs `codex plugin list --json`, canonicalizes it as jq did, writes `{"version":1,"status":…,"plugins":[…]}\n` atomically; ids sorted in UTF-16 order, `Array.prototype.sort`'s order) and `codex_plugin_installed` (`Installed`, `Absent`, `Unknown`).

**Config.** Ports of `config-files.ts`, `config-load.ts`, `config-read.ts`, `gate-mode.ts`, `quality-config.ts`, `docs-sync-config.ts`, `permissions.ts`, `settings*.ts`:
- `load(options)` reads `<TOOLU_USER_CONFIG_DIR or config root>/toolu.config.json` and `<project>/<dirname>/toolu.config.json`. A path that is not a regular file (following symlinks) is absent. A file that does not parse, or parses to `null` or `false` (`jq -e`), is ignored with the warning `malformed JSON in <path>; ignoring`; bytes are decoded lossily first, as `readFileSync(path, "utf8")` does.
- The envelope check reads the top-level keys **in document order** (an ordered visitor, not `serde_json::Map`, which sorts) and rejects, in this order: a non-object (`top level is not a JSON object`), unknown keys (`unknown top-level key 'a'` or `unknown top-level keys 'a', 'b'`, document order, a repeated key listed once at its first position), and a `version` that is not numerically 1 (`unsupported version <JSON.stringify(version)> (supported: 1)`; `1.0` is 1). The known keys are TypeScript's schema keys plus `epic`. A rejected file sets `invalid = "<path>: <reason>"`, adds the warning `<invalid>; failing closed (every gate blocks)`, and empties `data`; the user file's reason wins when both are invalid.
- `epic` is a namespaced section (#463): a known top-level key whose contents the loader never inspects. The same key is added to the TypeScript schema as `z.record(z.string(), z.unknown())`, so both loaders accept and reject the same files during coexistence (Jev `both`; brainstorm).
- `merge` is jq `$u * $p`: objects merge recursively, anything else on the project side replaces.
- `LoadedConfig { data, invalid, files, host, warnings }`. Resolvers append warnings to `warnings` (interior `RefCell`), which the caller drains with `take_warnings` and prints with the prefix `toolu-config: `.
- Resolvers keep every jq fallback: `section`, `enabled`, `flag_true`, `flag_false`, `enabled_explicit`, `config_string`, `model`, `codex_model`, `gate_preset`, `gate_mode` (with `supports_ask` degradation from `toolu-protocol`), `gate_decision`, `guardrail_warning`, `quality_threshold`, `ts_max_file_lines_resolved`, `quality_flag`, `native_max_lines`, `docs_sync_{surfaces,surface_excludes,code_surfaces}`. Non-string values in a message are printed as `JSON.stringify` prints them: numbers in JavaScript's `Number#toString` form (`1e21`, `1e-7`, `45.9`), `docsSync` items pretty-printed and split into lines.
- `permissions_autowrite(config, root, options)` ports the one-time Claude allowlist write (temp file created exclusively, then renamed; sentinel only after success; a settings file that cannot be parsed is left byte-identical).
- Settings: `settings_dir` (`TOOLU_SETTINGS_DIR`, `~/.claude/settings` when it is a directory, `<plugin root>/settings`), `read_list`, the six list files, `mcp_blocklist` (`prefix -> redirect`), `code_edit_rules` (strict rule objects).

**Process.** `process::run(&Spec) -> Result<Output, RunError>` runs one argv in its own process group (`CommandExt::process_group(0)`), feeds stdin from a thread (a closed pipe is not an error), drains stdout and stderr on two threads into one shared byte budget (both streams are always drained; bytes past the budget are dropped and `truncated` is set), and waits until the child exits and its group has no live member, or the deadline passes. On the deadline it sends `SIGTERM` to the group, waits 250 ms, sends `SIGKILL`, waits up to 1 s, and returns `timed_out`. `exit_code` is the status, or 128 + the signal. A group counts as alive when `killpg(pgid, None)` succeeds (or fails with `EPERM`) and `ps -axo pgid=,stat=` lists a non-zombie member (the probe result stands when `ps` fails). `signal_group`, `group_alive` and `terminate_group` are public. `git_toplevel(env, cwd)` and `codex_plugin_list(env)` are the two fixed commands the other modules use.

**Startup.** Ports of `publish.ts`, `context.ts`, `report.ts` and `dependencies.ts`:
- `publish(options) -> Published { result, warning, report_error }` symlinks `source` at `<config root>/<dir>/<name>`; the path is owned only when absent or a symlink, so a regular file or a directory is kept (`KeptUserFile`) and never touched. A missing source is `SourceMissing`, silently. An unwritable directory is `Unwritable` with the warning `<plugin>: cannot create <dir> — <what> not published`. The link is created beside the target under a random name and renamed over it, so readers never see the path missing.
- `report(env, record)` appends one JSON line to `TOOLU_STARTUP_REPORT` when it is set. The records keep TypeScript's key order: `registry {kind, spec, name, event, source, target, status, error?}`, `helper {kind, plugin, source, path?, status}`, `error {kind, origin, message}`. A failed append returns `toolu-startup: cannot write startup report <path>: <reason>`; the caller prints it and exits 1 after finishing its work.
- `session_context(event, text)` bounds the text to 10 000 UTF-16 units without splitting a surrogate pair; `render_hook_output(value, pretty)` prints jq's bytes (`JSON.stringify` plus `\u007f` for DEL).
- `codex_missing_plugins`, `requires_core_warning`, `requires_plugins_warning`, `codex_dependency_notice` port the Codex dependency warnings with their jq semantics.

**Registry types.** `RegistryEvent { ToolPre, ToolPost }` (`tool/pre`, `tool/post`; directories `pre-tools.d`, `post-tools.d`), `registry_root`, `event_dir`, `file_name(spec, name, ext)` and `parse_name(base)` (the TypeScript rules, extended with the `.json` manifest extension). `ModuleManifest` is the `<spec>__<name>.json` contract: `{"version":1,"spec":…,"name":…,"event":"tool/pre"|"tool/post","matcher":…}`, strict (unknown fields and any other version are rejected, Jev `strict` 0.87), and `read_manifest(path, event)` also checks that spec and name equal the file name and that the event equals the directory's. `matcher` is `*` (every tool) or `|`-separated entries, each an exact tool name or a prefix ending in `*` (`mcp__*`). The `Rule` trait (`spec`, `name`, `event`, `run(&NormalizedEvent, &RuleContext) -> Decision`) and `RuleContext` (host, env, config root, project root, cwd, raw payload, optional edit split with an `EditOperation` of `add`, `update`, `delete`, `write`, `move`) port `RegistryModule` and `RegistryContext`.

**CLI arguments.** `cli_args::jq_number(text)` accepts what `jq --argjson` read for a number (`numberValue`): optional ASCII whitespace around `[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?`, finite only. The quality thresholds reuse the same grammar after a JavaScript `trim`.

**Shared golden for config.** `fixtures/config/expected.json` holds one case per placement: each of the 16 fixtures as the project file on Claude, user+project merges, each fail-closed file as the user file, and inline texts for the boundaries (truncated, empty, `null`, `false`, a string version, `1.0`, two unknown keys out of alphabetical order, an `epic` section with unknown keys, `epic` next to an unknown key). Each case's `expect` holds `invalid` and the loader `warnings` (with `$USER_CONFIG`/`$PROJECT_CONFIG` for the absolute paths), the merged `data`, and `resolved`: a map from a call id (`gateMode pushReview claude`, `model review`, `qualityThreshold rust maxFnLines`, `docsSync codeSurfaces`, `enabled hooks pre-tools`, …) to `{value, warnings}`. It was captured once from the TypeScript implementation by a scratch script outside the repository, like `fixtures/host/encode.json`. `packages/toolu-core/src/config/__tests__/config-fixture.test.ts` and `crates/core/runtime/tests/config_fixture.rs` both reproduce every case; the suite is registered in `fixtures/index.json`.

**Host roots fixture.** `crates/core/runtime/tests/host_roots_fixture.rs` runs every case of `fixtures/host/root.json` against `Roots`, with the same sandbox (`$ROOT`, `$PROJECT`, a real `git init` for `git: true`) and `$runtime` values as `host-roots.test.ts`.

## Interfaces / Schema

```rust
// env.rs
pub struct Env(/* BTreeMap<String, String> */);
impl Env {
  pub fn process() -> Env;
  pub fn from_pairs<I, K, V>(pairs: I) -> Env where I: IntoIterator<Item = (K, V)>, K: Into<String>, V: Into<String>;
  pub fn get(&self, key: &str) -> Option<&str>;           // empty = unset
  pub fn vars(&self) -> impl Iterator<Item = (&str, &str)>;
  pub fn home(&self) -> PathBuf;
}

// host.rs
pub struct Detected { pub host: Host, pub warning: Option<String> }
pub fn detect(env: &Env, hook_event_name: Option<&str>) -> Detected;
pub struct Roots { /* env, host, warning */ }
impl Roots {
  pub fn new(env: Env, host: Option<Host>) -> Roots;
  pub fn host(&self) -> Host;  pub fn env(&self) -> &Env;  pub fn warning(&self) -> Option<&str>;
  pub fn config_root(&self) -> PathBuf;
  pub fn project_root(&self, cwd: Option<&Path>) -> Option<PathBuf>;
  pub fn project_dirname(&self) -> String;
  pub fn project_config_path(&self, cwd: Option<&Path>) -> Option<PathBuf>;
  pub fn project_state_root(&self, cwd: Option<&Path>, root: Option<&Path>) -> Option<PathBuf>;
  pub fn project_state_dir(&self, name: &str, cwd: Option<&Path>, root: Option<&Path>) -> Result<Option<PathBuf>, CallerError>;
  pub fn plugin_root(&self) -> Option<PathBuf>;  pub fn plugin_data(&self) -> Option<PathBuf>;
  pub fn invocation(&self, namespace: &str, name: &str) -> Result<String, CallerError>;
  pub fn plugin_install_command(&self, spec: &str) -> Result<Option<String>, CallerError>;
  pub fn plugin_uninstall_command(&self, name: &str) -> Result<Option<String>, CallerError>;
}
pub enum Installed { Installed, Absent, Unknown }
pub fn codex_plugin_snapshot_path(roots: &Roots) -> PathBuf;
pub fn snapshot_codex_plugins(roots: &Roots) -> Option<SnapshotResult>;   // None off Codex
pub fn codex_plugin_installed(spec: &str, roots: &Roots) -> Installed;

// config.rs
pub struct ConfigOptions<'a> { pub env: &'a Env, pub host: Option<Host>, pub cwd: Option<&'a Path> }
pub struct LoadedConfig { pub data: Map<String, Value>, pub invalid: Option<String>, pub files: ConfigFiles, pub host: Host, /* warnings */ }
pub fn load(options: &ConfigOptions<'_>) -> LoadedConfig;
pub fn exists(options: &ConfigOptions<'_>) -> bool;
pub fn merge(user: &Value, project: &Value) -> Value;
pub enum GateMode { Block, Ask, Advise, Off }   pub enum GatePreset { Strict, Balanced, Relaxed }
pub fn gate_mode(config: &LoadedConfig, name: &str, host: Option<Host>, event: Option<HostEvent>) -> GateMode;
pub fn quality_threshold(config: &LoadedConfig, lang: QualityLang, key: &str, root: Option<&Path>) -> u64;
// … the other resolvers named in Architecture, with the TypeScript names in snake_case

// process.rs
pub struct Spec { pub argv: Vec<String>, pub cwd: Option<PathBuf>, pub env: Option<Env>, pub stdin: Vec<u8>, pub timeout: Duration, pub max_output_bytes: usize }
pub struct Output { pub stdout: String, pub stderr: String, pub exit_code: i32, pub duration: Duration, pub timed_out: bool, pub truncated: bool }
pub fn run(spec: &Spec) -> Result<Output, RunError>;   // Err: empty argv, zero timeout, spawn failure

// startup.rs
pub enum PublishStatus { Published, KeptUserFile, LinkFailed, Unwritable, SourceMissing }
pub fn publish(options: &PublishOptions<'_>) -> Published;
pub fn report(env: &Env, record: &StartupRecord) -> Result<(), String>;

// registry.rs
pub enum RegistryEvent { ToolPre, ToolPost }
pub struct ModuleManifest { pub version: u32, pub spec: String, pub name: String, pub event: RegistryEvent, pub matcher: String }
pub trait Rule: Send + Sync {
  fn spec(&self) -> &str;  fn name(&self) -> &str;  fn event(&self) -> RegistryEvent;
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision;
}
```

`fixtures/config/expected.json`:

```json
{ "version": 1, "cases": [
  { "name": "fail-closed-unknown-key.json as the project file",
    "host": "claude",
    "user": null,
    "project": { "file": "fail-closed-unknown-key.json" },
    "expect": {
      "invalid": "$PROJECT_CONFIG: unknown top-level key 'nope'",
      "warnings": ["$PROJECT_CONFIG: unknown top-level key 'nope'; failing closed (every gate blocks)"],
      "data": {},
      "resolved": { "gateMode pushReview claude": { "value": "block", "warnings": [] } } } } ] }
```

A placement is `{"file": "<fixture>"}` or `{"text": "<raw file text>"}`.

## Failure modes and edge cases

- **Config file absent, a directory, or unreadable:** absent or a directory is silent and empty; unreadable or unparsable is the `malformed` warning; `null`/`false` too. Invalid UTF-8 decodes lossily, then parses or is malformed.
- **Envelope rejected:** `invalid` set, `data` empty, every `gate_mode` returns `block`, `permissions_autowrite` skips with `config invalid`.
- **Known divergences from `JSON.parse`, recorded rather than fixed:** serde_json rejects a lone-surrogate escape (`"\ud800"`) and a number outside `f64` (`1e400`), which JavaScript accepts; such a file reads as malformed, not as data. A non-string `version` object with two or more keys prints its keys sorted. None occurs in a fixture.
- **Wrong-typed values:** each resolver falls back exactly as its jq filter does, with the same warning text; an unknown gate name warns and blocks; an unknown model class is a caller error (`Err`), TypeScript's `TypeError`.
- **Empty names:** `project_state_dir("")`, `invocation("", …)`, `plugin_install_command("")` return `CallerError`, TypeScript's `TypeError`.
- **`git` missing, failing or outside a repository:** `project_root` is `None`. `codex` missing or failing: the snapshot is `indeterminate` and dependency checks are skipped.
- **Subprocess:** an empty argv or zero timeout is `Err` before spawning; a spawn failure is `Err`; a child that ignores `SIGTERM` is killed with `SIGKILL`; a grandchild that keeps the pipes open past the deadline is killed with its group, and the output gathered so far is returned; stdin to a child that closes it early is not an error.
- **Publish:** source missing → nothing written; existing regular file or directory → kept; symlink to the same source → `Published` without a write; stale or dangling symlink → replaced atomically; link failure → `LinkFailed` and no temp file left behind.
- **Startup report:** unset → nothing written; an unwritable path → the error line is returned, never swallowed.
- **Concurrent writers:** two sessions publishing the same helper, or writing the Codex snapshot, each rename a private temp file over the target, so the last rename wins and no reader sees a torn or missing file. The permissions write uses an exclusive temp file, and its sentinel makes a second session skip.
- **Manifest:** unknown field, version ≠ 1, spec/name differing from the file name, event differing from the directory, an empty matcher → `Err` naming the file.

## Acceptance criteria

- **AC-1:** Every case of `fixtures/config/expected.json` — each of the 16 `fixtures/config/*.json` files and the inline boundary texts — gives the same `invalid` text, loader warnings, merged data, resolved values and resolver warnings under the Rust loader as under the TypeScript loader, including `unknown top-level key 'nope'`, `unsupported version 2 (supported: 1)` and `top level is not a JSON object`.
- **AC-2:** Every case of `fixtures/host/root.json` gives the same value, absence or caller error under `Roots` as under `host-roots.ts`, and the invalid-override case yields its warning exactly once per resolution.
- **AC-3:** A config whose `epic` section holds unknown keys loads with `invalid` unset under both loaders, while the same file with an extra unknown top-level key fails closed with the unknown-key message.
- **AC-4:** With `TOOLU_CONFIG_DIR` set and `PLUGIN_ROOT` set (Codex), the registry directory and the Codex snapshot path resolve under `TOOLU_CONFIG_DIR`, not `CODEX_HOME`.
- **AC-5:** `cargo tree -p toolu-runtime` lists no `tree-sitter`, `brush`, `rustls` or `ureq` crate.
- **AC-6:** Publishing over a user's regular file leaves its bytes and type unchanged and reports `kept-user-file`; publishing over a stale symlink replaces it; both outcomes are appended to `TOOLU_STARTUP_REPORT` with TypeScript's key order.
- **AC-7:** `process::run` of a real shell child that sleeps past its deadline and spawns a grandchild returns `timed_out` within the grace period, and afterwards no process of its group is alive; a child that writes more than the budget returns `truncated` with exactly the budget's bytes.
- **AC-8:** A registry manifest with an unknown field, a version other than 1, or a name that differs from its file name is rejected; a valid one round-trips and its matcher selects exactly the listed tools.
- **AC-9:** `cargo xtask gate` passes for the workspace with no exemption, and `bun run test` passes the fixture inventory and the new TypeScript fixture test.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | the 16 committed fixtures and the inline texts in `expected.json` | every `expect` reproduced | malformed texts, string and `1.0` versions, two unknown keys, `edge-shapes.json` | `cargo test -p toolu-runtime --test config_fixture`; `bun test packages/toolu-core/src/config/__tests__/config-fixture.test.ts` |
| AC-2 | `fixtures/host/root.json` (18 cases), real `git init` sandboxes | every `expected` reproduced | no git repo, `GIT_CEILING_DIRECTORIES`, unset `HOME`, empty names | `cargo test -p toolu-runtime --test host_roots_fixture` |
| AC-3 | the golden's `epic` cases | valid / fails closed | `epic` next to `nope` | the two AC-1 tests |
| AC-4 | env `{HOME, CODEX_HOME, PLUGIN_ROOT, TOOLU_CONFIG_DIR}` | `<TOOLU_CONFIG_DIR>/toolu/pre-tools.d` and `<TOOLU_CONFIG_DIR>/toolu/codex-plugins.json` | without `TOOLU_CONFIG_DIR` they sit under `CODEX_HOME` | `cargo test -p toolu-runtime --lib -- registry::tests::toolu_config_dir_wins_on_codex host::snapshot::tests::toolu_config_dir_wins_on_codex` |
| AC-5 | the workspace lockfile | no banned crate | — | `! cargo tree -p toolu-runtime -e normal --prefix none \| grep -E '^(tree-sitter\|brush\|rustls\|ureq)'` |
| AC-6 | a temp config root with a user's `jev.sh` regular file, then a dangling symlink | file unchanged, `kept-user-file`; symlink replaced, `published`; two report lines | missing source, directory at the path | `cargo test -p toolu-runtime --lib -- startup::` |
| AC-7 | `sh -c 'sleep 30 & sleep 30'` with a 200 ms deadline; `head -c 4096 /dev/zero` with a 100-byte budget | `timed_out`, group dead; `truncated`, 100 bytes | a child that closes stdin early; exit-by-signal code 128+n | `cargo test -p toolu-runtime --lib -- process::` |
| AC-8 | manifest texts in a temp `pre-tools.d` | rejected / accepted as listed | `mcp__*` prefix, `*` | `cargo test -p toolu-runtime --lib -- registry::` |
| AC-9 | the branch | green | — | `cargo xtask gate`; `bun run test` (environmental failures of comemory `be52369e` baselined on `origin/main`) |

## Documentation impact

- `docs/config.md`: the `epic` key — reserved for the epic engine (#463); its contents are not checked by the config loader.
- `fixtures/README.md` and `fixtures/config/README.md`: the new `expected.json` golden, its consumers and how it was captured; the suite table row.
- `AGENTS.md` **Key files**: a `crates/core/runtime/src/lib.rs` row naming the modules and their TypeScript sources.
- `crates/core/runtime` crate `//!` doc updated.

## Open Questions

None blocking. The launcher-script owned set (Non-Goal 6) is #443's decision; the strict schema (Non-Goal 2) is #462's.

## Spec review

Round 1 (Jev: issue coverage 0.55, Acceptance-section deviation 0.34, evidence gaps 0.72; agent mapping: the 5 issue checkboxes map to AC-1, AC-2, AC-3, AC-5, AC-6 and the 2 scenarios to AC-1 and AC-4):
- Acceptance evidence: 🟡 should-fix: AC-4 and AC-5 named their checks loosely. Fixed with exact commands and test names.
- Failure modes: 🟡 should-fix: concurrent publishers and snapshot writers were not stated. Added.
- Non-Goals: 🔵 consider: Non-Goal 6 narrows the issue's Scope wording ("or the launcher script"), not its Acceptance; recorded with its owner (#443).

Round 2: no findings. **Status:** Approved.
