# toolu-state: gate file, telemetry, edit records and git facts without spawning git — Design

**Date:** 2026-10-06   **Status:** Draft   **Author:** Claude (epic #402 worker)   **Topic:** Port `@toolu/core/state` and the shell-free half of `@toolu/core/detect` to `crates/core/state`, and read the per-call git facts from `.git` (#415)

## Problem

Until #437 and #426–#428 land, TypeScript and Rust hooks write the same gate file, `<project>/<host dir>/tmp/quality-gate-status.json`. If the two writers differ in format, key order or lock protocol, one drops the other's entries or fails the strict schema, and a failing gate silently reads as passing. Separately, a CPU profile of `pre-tools.js` put about half its samples in `git rev-parse --show-toplevel`, spawned twice per hook. `toolu-runtime`'s `Roots::project_root` (#414) does the same spawn on every hook outside Claude, which by itself breaks the epic's 5 ms CPU budget. The Rust hooks of #418–#424 need the state layer and these git facts first.

## Non-Goals

1. No hook, verb or `hooks.json` entry is ported or changed; the `toolu` binary is untouched. Consumers are #418–#424, #445, #459.
2. Detection from a parsed command (`isGitPush`, `isGitCommit`, `pushTargetRoot`, `pushTargetBranch`) moves to #418.
3. The TypeScript state layer keeps its behaviour. It gains only a Bun test that asserts the new shared goldens.
4. Commit ancestry (`git branch --merged`), the index (`git ls-files`) and diffs are not reimplemented. Those session-time queries keep spawning git (decision D2).
5. No OS advisory lock (`fs4`): it would not exclude TypeScript writers.
6. No change to gate data (`[workspace.lints]`, `clippy.toml`, `rules.json`, …).

## Architecture

**Decisions** (brainstorm `docs/toolu/brainstorms/2026-10-06-toolu-state.md`):

- **D1 — discovery in runtime (Jev 0.91).** The `.git` walk lives in `toolu-runtime`, as a new `git` module, because `Roots::project_root` and `config::quality` (runtime) need it and runtime cannot depend on state. Both stop spawning `git rev-parse --show-toplevel`. `toolu-state::git` builds the branch, linked-worktree, common-dir and origin-HEAD answers on the discovery and re-exports `toplevel`.
- **D2 — which git questions spawn (Jev 0.98).** Per-call facts never spawn. These session-time history and index queries still spawn git through `toolu_runtime::process::run`:
  - `diff_sha` (`git diff`, `git hash-object`);
  - the sweeper's branch lists (`git branch --format=%(refname:short) [--merged <base>]`, plus `git --version`);
  - `detect_ts` (`git ls-files`).
  
  This reads the issue's "git is spawned only for diffs" as applying to the facts it lists. Doing better needs a git object and index library (`gix`).
- **D3 — Rust writers in the interleave test are real processes (Jev 0.93).** `tests/interleave.rs` is `harness = false`. Its `main` runs the scenarios and executes itself as each Rust writer, beside real `bun packages/toolu-core/src/state/__tests__/gate-writer.ts` processes. Precedent: `crates/core/protocol/tests/run_hook.rs`.
- **D4 — parity oracles (Jev `mixed` 0.98):**
  - Gate-file bytes and edit records get new committed goldens, captured once from TypeScript and asserted by a Bun test and a Rust test: `fixtures/state/gate-bytes.json` and `fixtures/state/edit-records.json`.
  - Detect layouts and line-count snippets are ported into Rust tests with the values the TypeScript tests assert.
  - One Rust test compares Rust and TypeScript line counts over every tracked source through a live `bun` call.
  - The 77 existing `fixtures/state/cases.json` cases run against Rust too, all except `package`, which checks TypeScript module exports.
- **D5 — strict v1 validation by hand** over document-order JSON (`toolu_runtime::json::ordered::Ordered`), with zod's `<path>: <message>` reason. serde's `deny_unknown_fields` is not used: it would drop key order, and it rejects `1.0` where `JSON.parse` sees `1`.
- **D6 — JavaScript key order.** Gate entries are ordered as a JavaScript object orders them: array-index keys (canonical decimal below 2³²−1) first in ascending order, then the other keys in insertion order. This keeps the pinned divergence of the `ordering` fixture byte-identical to TypeScript.

**Reuse:**
- `toolu_runtime::json` for `JSON.stringify` and jq text (`Ordered`, `jq_text`, `js_number`);
- `toolu_runtime::host::roots::Roots` for state and telemetry dirs;
- `toolu_runtime::config::{load::load, read::enabled}` for the `telemetry.enabled` and `gates.sweep` switches and the TTL settings;
- `toolu_runtime::process` for every spawn.

`tempfile` (already a workspace dependency) supplies the 0600 temp file. `nix` (already a workspace dependency) supplies `kill(pid, 0)` for liveness, and its `user` feature, enabled in `toolu-state`'s manifest only, supplies `geteuid` for the ownership check.

**Discovery algorithm** (`toolu_runtime::git::discover`), mirroring git's `setup_git_directory_gently_1`:
1. If `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_CEILING_DIRECTORIES` or `GIT_DISCOVERY_ACROSS_FILESYSTEM` is set in the `Env`, return `AskGit`.
2. Canonicalize `cwd` (git works from the physical cwd). If that fails, return `NotFound`.
3. For each directory D from cwd upward:
   1. If `D/.git` is a file, it must read `gitdir: <path>` (resolved against D) and name a valid git directory. Otherwise `NotFound`: git dies on an invalid gitfile.
   2. If `D/.git` is a directory and a valid git directory, it is the git dir.
   3. On a hit, the common dir is the git dir's `commondir` file (resolved against the git dir) when present, else the git dir. The toplevel is D.
   4. Otherwise, if D itself is a valid git directory (bare repository, or inside a `.git`), return a repository with no toplevel.
   5. Stop with `NotFound` at `/`, or when the parent lies on another device than cwd (git's filesystem boundary).
4. A valid git directory has a valid `HEAD` and `objects/` and `refs/` under its common dir. A valid `HEAD` is a symlink to `refs/…`, `ref: refs/…`, or a hex object id.
5. **AskGit cases:** a found repository returns `AskGit` instead when:
   - D, the gitfile or the git dir is owned by another uid than the effective one (`safe.directory`);
   - the git dir's `config` sets `core.worktree` or `core.bare = true`;
   - a `config.worktree` file exists.

`toplevel(env, cwd)` maps `Repo` to its toplevel, `NotFound` to `None`, and `AskGit` to the existing spawning `process::commands::git_toplevel`.

**Branch** (`toolu_state::git::current_branch`), `git rev-parse --abbrev-ref HEAD` semantics:
- not a repository → `""`; detached or unborn → `"HEAD"`; `ref: refs/heads/<name>` with an existing ref (loose or `packed-refs`) → `<name>`.
- `AskGit`, a reftable repository, a `HEAD` symlink, a `ref:` outside `refs/heads/`, or a short name that git would lengthen → spawn git as TypeScript does. Git lengthens the short name when `<name>`, `refs/<name>`, `refs/tags/<name>`, `refs/remotes/<name>` or `refs/remotes/<name>/HEAD` exists.

## Interfaces / Schema

```rust
// toolu-runtime (new module `git`; `process::Output` gains `stdout_bytes`)
pub enum Discovery { Repo(Repo), NotFound, AskGit }
pub struct Repo { pub toplevel: Option<PathBuf>, pub git_dir: PathBuf, pub common_dir: PathBuf }
pub fn discover(env: &Env, cwd: &Path) -> Discovery;
pub fn toplevel(env: &Env, cwd: &Path) -> Option<PathBuf>;
pub struct Output { /* existing fields */ pub stdout_bytes: Vec<u8> }   // raw stdout within the budget

// toolu-state
pub struct StateCtx { pub roots: Roots, pub config: Option<LoadedConfig>, pub now: Option<SystemTime>, pub warnings: Vec<String> }
// time / io / lock
pub fn iso_seconds(t: SystemTime) -> String;                       // 2026-09-28T12:34:56Z
pub fn write_atomic(path: &Path, body: &str) -> bool;              // 0600 temp beside, fsync, persist, fsync dir
pub struct LockOptions { pub timeout: Duration /*5 s*/, pub stale: Duration /*2 s*/ }
pub fn with_lock<T>(file: &Path, opts: LockOptions, warnings: &mut Vec<String>, f: impl FnOnce() -> T) -> T;
// schema
pub const GATE_FILE_VERSION: u8 = 1; pub const GLOBAL_GATE_KEY: &str = "__global__";
pub struct GateEntry { pub source: String, pub reason: String, pub violations: String, pub updated_at: String }
pub enum GateFile { Passing { source: String, updated_at: String },
                    Failing { reason: String, source: String, file: String, violations: String,
                              entries: Option<Vec<(String, GateEntry)>>, updated_at: String } }
pub fn validate_gate_file(value: &Ordered) -> Result<GateFile, String>;  // "<path>: <message>"
// gate file
pub enum GateRead { Missing, Malformed(String), Unrecognized { reason: String, value: Ordered }, Ok(GateFile) }
pub fn read_gate_file(gate: &Path) -> GateRead;
pub fn record_gate_failure(ctx: &mut StateCtx, gate: &Path, file: &str, source: &str, reason: &str, violations: &str);
pub enum ClearOutcome { Cleared, Noop }
pub fn clear_gate_file(ctx: &mut StateCtx, gate: &Path, file: &str, source: &str) -> ClearOutcome;
// telemetry (closed: one variant per event, exactly its extras, in TELEMETRY_EXTRAS key order)
pub enum TelemetryEvent { GateFail { file, source }, GateClear { file, source },
  StepRun { step_id, status, exit_code: f64, duration_s: f64, attempt: f64 },
  AcCoverage { covered: f64, uncovered: f64 }, DocsAttested { decision }, DocsNudge,
  PushCheck { result, reason_code, round: Option<f64> },
  Delegation { model, subagent_type, reasoning_effort, step_id, step_model: Option<String> } }
pub const TELEMETRY_VERSION: u8 = 1; pub const TELEMETRY_MAX_LINE_BYTES: usize = 3900;
pub enum TelemetryResult { Written(PathBuf), Skipped(String) }
pub fn telemetry_append(ctx: &mut StateCtx, root: &Path, event: &TelemetryEvent) -> TelemetryResult;
pub fn parse_telemetry_line(value: &Ordered) -> Result<TelemetryEvent, String>;   // strict line schema
pub fn parse_telemetry_extras(event: &str, value: &Ordered) -> Result<TelemetryEvent, String>;
// edit records
pub enum EditOperation { Add, Update, Delete, Write, Move }
pub struct EditRecord { pub path: String, pub operation: EditOperation, pub moved_to: Option<String>, pub from: Option<String> }
pub enum EditRecords { Records(Vec<EditRecord>), NotEdit, Malformed }
pub fn is_edit_tool(tool: &str) -> bool;
pub fn apply_patch_records(patch: &str) -> Option<Vec<EditRecord>>;
pub fn normalize_edit_records(payload: &serde_json::Value, tool: &str) -> EditRecords;
pub fn format_edit_records(records: &[EditRecord]) -> String;            // one `jq -c` object per line
// sweeper, diff
pub fn sweep_state(ctx: &mut StateCtx, root: Option<&Path>);
pub fn slug_of_state_file(file: &Path) -> String;
pub fn kept_telemetry_lines(content: &str, cutoff: &str) -> Option<Vec<String>>;
pub fn diff_sha(env: &Env, repo_root: &Path, base_ref: &str) -> Option<String>;
// git facts
pub use toolu_runtime::git::toplevel;
pub fn current_branch(env: &Env, root: &Path) -> String;
pub fn linked_worktree(env: &Env, dir: &Path) -> bool;
pub fn common_dir(env: &Env, dir: &Path) -> Option<PathBuf>;
pub fn base_branch(env: &Env, root: Option<&Path>, cwd: &Path) -> String;
pub fn branch_slug(branch: &str) -> String;
pub fn branch_slugs(env: &Env, root: &Path, merged_into: Option<&str>) -> BTreeSet<String>;
pub fn has_git(env: &Env) -> bool;
// detect (cwd-based probes take env + cwd)
pub fn project_toplevel / project_name / node_package_manager / detect_rust / detect_python /
       detect_ts / ts_linter / python_linter / detect_clippy / to_relative_path;
pub fn tool_available(name: &str, env: &Env) -> bool; pub fn detect_ast_grep(env: &Env) -> bool;
pub fn count_code_lines(path: &Path) -> Option<u64>; pub fn count_python_code_lines(path: &Path) -> Option<u64>;
pub fn has_unterminated_block(path: &Path) -> bool;
```

**On-disk shapes:**
- **Gate file:** two-space `JSON.stringify` plus `"\n"`, DEL escaped as `\u007f`. Failing key order: `status, reason, source, file, violations, entries, updatedAt`; entry order: `source, reason, violations, updatedAt`; passing: `status, source, updatedAt`.
- **Lock file** `<gate>.lock`: `"<pid> <uuid-v4>\n"`, created exclusively with mode 0600.
- **Telemetry line:** `{...extras, "v":1, "t", "branch", "event"}` compact, plus `"\n"`, appended to `<state root>/telemetry/<branch_slug>.jsonl` or to `$TELEMETRY_DIR`.
- **Drop log:** `<gate>.dropped.log` lines as in TypeScript.

**New goldens** (`{ "version": 1, "cases": [...] }`, indexed in `fixtures/index.json`):
- `fixtures/state/gate-bytes.json`: `{ name, initial: string|null, steps: [{ op: "record"|"clear", file, source, reason?, violations?, now, expect: { bytes: string|null, outcome?, dropLog? } }] }`;
- `fixtures/state/edit-records.json`: `{ name, tool, payload, expect: { kind: "records"|"not-edit"|"malformed", records?, text? } }`.

## Failure modes and edge cases

- **Gate read:**
  - missing → `Missing`;
  - empty, unparseable, `null`, `false` → `Malformed`;
  - parses but fails v1 (unknown key at any level, `version` ≠ 1, unknown status, wrong type, `entries` not an object) → `Unrecognized` with the first offending path.
  - Recording over `Unrecognized` replaces it, warns `gate-file: unrecognized gate file at <gate> (<reason>); replacing it` and appends a drop-log line. Clearing leaves it byte-identical, warns `…; ignoring clear`, and emits no telemetry.
- **Clear of a malformed file:** warns `gate-file: malformed JSON at <gate>; ignoring clear (gate stays failing until next write)`. Clear of a missing file is a silent `Noop`. A clear with nothing to clear decides from an unlocked read and never waits on a live lock.
- **Locking:**
  - The lock is taken by exclusive create.
  - If it is busy, the holder is judged and the call polls every 10 ms. A holder is dead when `kill(pid, 0)` reports `ESRCH`, and stale when the lock is older than 2 s. A dead or stale holder is broken by renaming the lock to `<lock>.<uuid>.broken`; if the claimed content differs, it is linked back, then removed.
  - After 5 s the call warns `state: lock <lock> still held; writing without it` and runs unlocked.
  - A lock that cannot be created (missing directory) runs the closure unlocked, without warning.
  - Release removes the lock only if it still holds our content. Release runs from `Drop`, so a panicking closure still releases (TypeScript `finally`).
- **Atomic write:** a missing directory or any failed step returns `false` and leaves no temp file. Recording then falls back to a single-slot write, warning and logging if that drops other entries, as TypeScript does.
- **Telemetry** skips with a reason, never an error:
  - empty root;
  - config `telemetry.enabled: false`;
  - no branch (`""` or `"HEAD"`);
  - an assembled line over 3,900 bytes (with a warning);
  - no state dir;
  - a failed append.
- **Edit records:**
  - A malformed payload yields `Malformed`, never partial records: a non-object payload or `tool_input`, no path, a path holding `\n`, `\r` or `\t`, or a patch with no headers, an unknown `*** ` header, a header before `*** Begin Patch`, a second Begin, text after End, or a Move without a pending Update.
  - A trailing CR is stripped per line, and trailing newlines are stripped from substituted strings.
- **Sweeper:**
  - It never panics outward; failures become `toolu-sweep: …` warnings.
  - It skips everything when `gates.sweep` is false, git is missing, or the state root is absent.
  - It never touches the current branch's files, an unrecognized gate file, or a failing gate with a live (or `__global__`) entry.
  - A telemetry file with an unparseable line is left alone.
- **`diff_sha`:**
  - A base ref starting with `-` gives `None` without spawning.
  - A failing `git diff` or `hash-object`, or an empty hash, gives `None`.
  - Diffs over 1 MiB, and non-UTF-8 diffs, hash exactly: raw stdout bytes are piped with no output budget.
- **Discovery:**
  - A nonexistent cwd gives `NotFound`.
  - An invalid `.git` file gives `NotFound` (git dies on it).
  - A `.git` directory that is not a valid git dir is skipped, and the walk continues upward (git's behaviour).
  - The `GIT_*` variables, foreign ownership, `core.worktree`, `core.bare`, reftable and ambiguous short names defer to git (D1). With `PATH` empty, those deferrals answer `None` or `""`, exactly as TypeScript does when git is absent.
- **Concurrency:** TypeScript and Rust writers share the lock, so no slot is lost. Readers see the old or the new document, never a torn one, because every write is a rename.

## Acceptance criteria

- **AC-1:** Eight TypeScript and eight Rust writer processes race on one gate file. Each records `/w/<id>/0..3` and then clears the even indices. After all exit:
  - every odd entry from all 16 writers is present, and the file parses under the strict schema;
  - a reader polling throughout saw only `Ok` or `Missing`;
  - no `.tmp` or `.lock` file remains;
  - no writer warned "still held";
  - each Rust writer's slowest single operation took under 2 s.
- **AC-2:** A lock left by a crashed Rust writer (dead pid, our format) is broken by a TypeScript writer within 2 s, and a lock left by a crashed TypeScript writer is broken by a Rust writer within 2 s. Both writers' entries land.
- **AC-3:** For every case of `fixtures/state/gate-bytes.json`, applying its steps with the Rust API produces exactly the expected file bytes, outcomes and drop-log bytes after each step. The Bun test produces the same with TypeScript. The cases cover:
  - a fresh record;
  - a second slot;
  - a re-record in place;
  - clear-with-promotion and clear-to-passing;
  - a legacy single-slot seed, and a `version: 1` seed;
  - an integer-like key;
  - DEL, control and non-ASCII text;
  - equal timestamps ordered by key bytes;
  - a non-owner clear;
  - an unrecognized replacement.
- **AC-4:** Every `fixtures/state/cases.json` case except the TypeScript-only `package` case passes against the Rust implementation: schema, telemetry events and extras, gate-file, io (jq text checked against real `jq`), branch-slug, base-branch, current-branch, branch-slugs, diff-sha, and concurrency with Rust writers.
- **AC-5:** On repositories created by the test, `toplevel`, `current_branch`, `linked_worktree` and `common_dir` equal `git rev-parse --show-toplevel`, `--abbrev-ref HEAD`, and `--git-dir`/`--git-common-dir` (absolute) in each layout:
  - a normal repo, from its root and from a subdirectory reached through a symlink;
  - a linked worktree;
  - a submodule;
  - a detached HEAD, and an unborn branch;
  - a branch held only in `packed-refs`;
  - a `--separate-git-dir` `.git` file pointer;
  - a bare repository, and inside `.git`;
  - a directory outside any repository;
  - a branch name that is also a tag (deferred to git).
- **AC-6:** With `PATH` set to the empty string, the following give the same answers as AC-5 for the normal repo, the linked worktree, the submodule and the gitfile pointer, proving no process was spawned:
  - `toolu_state::git::toplevel`, `current_branch` and `linked_worktree`;
  - `toolu_runtime::host::roots::Roots::project_root`.
- **AC-7:** `cargo tree -p toolu-state -e normal` lists none of the crates `rules.json` reserves for `toolu-shell` (shell parsers) or `toolu-http` (HTTP/TLS).
- **AC-8:** For every case of `fixtures/state/edit-records.json`, `normalize_edit_records` and `format_edit_records` produce the expected kind, records and text. The Bun test produces the same with TypeScript. The cases cover:
  - Edit, Write, MultiEdit;
  - `file_path`, `path` and `target_file` precedence;
  - apply_patch add, update, delete and move, with EOF marker and CRLF;
  - each malformed form;
  - a non-edit tool.
- **AC-9:** `telemetry_append`:
  - writes a byte-exact line for each of the eight events (the `telemetry-events` fixture lines) to `<branch_slug>.jsonl`, or to `$TELEMETRY_DIR`;
  - writes nothing for a detached HEAD, `telemetry.enabled: false`, or a line over 3,900 bytes (with a warning).
  
  `record_gate_failure` and `clear_gate_file` emit `gate_fail` and `gate_clear` under the gate file's root.
- **AC-10:** `sweep_state` on a real repository:
  - removes branch-state files of merged and deleted branches, and of a live branch older than the TTL;
  - keeps the current branch's files and a recent live one;
  - removes a passing gate file and a failing one whose entries name only missing files;
  - keeps a failing one with a live file or `__global__`, and keeps an unrecognized one;
  - trims telemetry to the retention window, deletes an emptied file, and keeps a file with an unparseable line byte-identical;
  - does nothing when `gates.sweep` is false.
- **AC-11:** `diff_sha` reproduces the six `diff-sha` fixture cases, and also hashes a diff with non-UTF-8 bytes to the same id as `git diff --no-color B...HEAD | git hash-object --stdin`.
- **AC-12:** The detect probes match what the TypeScript tests assert:
  - the 35 project layouts from the root and a subdirectory, and the bats answers;
  - `tool_available` and `detect_ast_grep` over isolated `PATH` directories, with a non-executable file, a directory, a dangling link, a name with `/` and an empty `PATH` entry;
  - the line counters on the snippet table, a missing file, a directory, NUL bytes and a file over 1 MiB.
  
  `count_code_lines`, `count_python_code_lines` and `has_unterminated_block` also agree with TypeScript on every tracked `*.ts`, `*.rs`, `*.py`, `*.sh` file of the repository.
- **AC-13:** `cargo xtask gate` passes with no exemption: `toolu-state` coverage is at least 90% and `toolu-runtime` stays at 85% or more. `bun run test` passes, apart from this host's known environmental failures (comemory `be52369e`), which are checked against `origin/main`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | 8 `bun gate-writer.ts` + 8 self-exec Rust writers, real git sandbox, polling reader | all odd entries, strict-valid reads only, no leftovers, no timeout warnings, max op < 2 s | 16 concurrent processes on one lock | `cargo test -p toolu-state --test interleave` |
| AC-2 | crashed-holder locks written by a Rust child that exits inside the lock and by a `bun -e` child that exits inside `withLock` | other implementation proceeds < 2 s; both entries present | dead pid, fresh mtime | same |
| AC-3 | `fixtures/state/gate-bytes.json` | byte-equal after each step | integer keys, DEL, equal timestamps, unrecognized | `cargo test -p toolu-state --test gate_bytes`; `bun test packages/toolu-core/src/state/__tests__/gate-bytes.test.ts` |
| AC-4 | `fixtures/state/cases.json` (76 of 77) | each case's expectation | jq byte text vs real `jq` | `cargo test -p toolu-state --test state_cases` (+ the concurrency cases in `interleave`) |
| AC-5 | repos built with real `git init/worktree/submodule/checkout --detach/pack-refs/init --separate-git-dir/init --bare` | equal to `git rev-parse` output | symlinked cwd, unborn, ambiguous tag | `cargo test -p toolu-state --test git_facts` |
| AC-6 | same repos, `Env` with `PATH=""` | same answers | no git reachable | same |
| AC-7 | `cargo tree -p toolu-state -e normal --prefix none --offline` | no reserved crate | transitive deps | `cargo test -p toolu-state --test deps` |
| AC-8 | `fixtures/state/edit-records.json` | kind/records/text equal | CRLF, every malformed form | `cargo test -p toolu-state --test edit_records`; `bun test packages/toolu-core/src/state/__tests__/edit-records.test.ts` |
| AC-9 | git sandbox with config, `telemetry-events` fixture lines | exact bytes; skips with reasons | detached, disabled, 3,901-byte line | `cargo test -p toolu-state` (unit `telemetry_test`) |
| AC-10 | git repo with merged/deleted/live branches, aged files (`set_modified`), gate files, telemetry files | the listed removals and keeps | unrecognized gate, bad JSONL | `cargo test -p toolu-state --test sweeper` |
| AC-11 | fixture repos + a commit holding bytes `\xff\xfe` | ids equal git's pipeline | >1 MiB, `-x` base | `cargo test -p toolu-state --test state_cases` and unit `diff_sha_test` |
| AC-12 | TS test tables ported; repository tracked sources | same values; Rust = TS counts per file | NUL, dir, missing, 1 MiB | `cargo test -p toolu-state --test detect` |
| AC-13 | whole workspace | green | — | `cargo xtask gate`; `bun run test` |

## Documentation impact

- `AGENTS.md` **Key files**: a new `crates/core/state/src/lib.rs` row; the `toolu-runtime` row names the new `git` discovery.
- `fixtures/README.md` and `fixtures/index.json`: the two new goldens, plus the Rust consumers of `state/cases.json`.
- `docs/detect.md`: a "Rust port" section covering where the shell-free probes live (`toolu_state::detect`), git facts read from `.git` with their deferral cases, and the spawns that remain (D2).
- Crate `//!` docs carry the format and lock contract.
- No user-facing command, config key or skill changes.

## Open Questions

None blocking. Resolved here:
1. Which git queries may spawn: D2.
2. Where discovery lives: D1.
3. What happens when `bun` is absent from the Rust test environment. The AC-1/2 and AC-12 corpus tests fail closed with a clear message rather than skipping. The CI rust job installs Bun and runs `bun install` (AGENTS.md CI table).
