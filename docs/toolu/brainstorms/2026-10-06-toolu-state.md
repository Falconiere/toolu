# toolu-state: gate file, telemetry, edit records and git facts without spawning git — brainstorm (#415)

**Date:** 2026-10-06   **Mode:** Delivery, Full path (a persisted format shared with TypeScript writers; a core crate's public interface)

## Capsule

- **Outcome:** `crates/core/state` reads and writes the multi-slot gate file byte for byte as `@toolu/core/state` does, under the same `<file>.lock` protocol, so TypeScript and Rust writers can share one file. It also appends closed-schema telemetry, normalizes edit payloads, sweeps spent state, hashes branch diffs, and answers the per-call git facts (toplevel, branch, linked worktree, common dir, origin HEAD) by reading `.git`. The shell-free detection of `@toolu/core/detect` comes with it: project markers, linters, tools on `PATH` and line counts.
- **Material defaults / non-goals:** no hook is ported and the `toolu` binary does not change; the consumers are #418 to #424, #445 and #459. Detection from a parsed command (`isGitPush`, `pushTargetRoot`, `pushTargetBranch`) moves to #418. The TypeScript state layer stays the reference and changes only where a shared golden asserts it.
- **Repository evidence:** `packages/toolu-core/src/state/*.ts` (1,207 lines), `packages/toolu-core/src/detect/*.ts` (493 lines), `fixtures/state/cases.json` (77 cases from #408), the TypeScript concurrency writer `packages/toolu-core/src/state/__tests__/gate-writer.ts`, `toolu_runtime::json::ordered::Ordered` (document-order JSON and `JSON.stringify` text), `toolu_runtime::process::commands::git_toplevel` (the spawn behind `Roots::project_root`), and the #413 self-executing `harness = false` test `crates/core/protocol/tests/run_hook.rs`.
- **Risk:** JavaScript object key order (integer-like keys first) has to be reproduced for byte parity. The `.git` walk has to agree with `git rev-parse` on symlinked paths, linked worktrees, submodules, `--separate-git-dir` pointers, bare and inside-`.git` directories, and filesystem boundaries. Where it cannot be sure (the `GIT_DIR` family of variables, `core.worktree`, foreign ownership, reftable, an ambiguous short branch name), it asks git.
- **Handoff:** spec.

## Axes and decisions

| Axis | Decision | Evidence | Jev |
|---|---|---|---|
| Where `.git` discovery lives | In `toolu-runtime` (`git` module): toplevel, git dir and common dir found by walking up from the physical cwd. `Roots::project_root` and `config::quality` use it in place of spawning `git rev-parse --show-toplevel`. `toolu-state`'s `git` module builds the branch, linked-worktree and origin-HEAD answers on it and re-exports the toplevel | runtime cannot depend on state (`layers.json`); config is loaded on every hook and resolves the project root through `Roots::project_root`, which spawns git on Codex, Hermes and OpenCode; epic CPU budget 5 ms p50 | `runtime_discovery` 0.91 |
| Which git questions still spawn | Only session-time history or index queries spawn: `diffSha` (`git diff`, `git hash-object`), the sweeper's live and merged branch lists (`git branch [--merged]`), and `detectTs` (`git ls-files`). Per-call facts never spawn | `--merged` needs commit ancestry (compressed objects, packfiles); `ls-files` needs the index (v2–v4, split and sparse); no git library is a workspace dependency; both run once per SessionStart | `hot_path_only` 0.98 |
| Rust writers in the interleave test | An integration test with `harness = false` whose `main` runs the race and executes itself as each Rust writer, beside real `bun gate-writer.ts` processes | Precedent `crates/core/protocol/tests/run_hook.rs`; capability rule 14 scans `src` only; no `#[ignore]` | `harness_false_self_exec` 0.93 |
| Byte parity oracle | A committed golden, `fixtures/state/gate-bytes.json`: operation sequences with the exact file bytes after each step, captured once from TypeScript. A Bun test and a Rust test both reproduce it | #413 `host/encode.json` and #414 `config/expected.json` precedents; the existing gate-file cases compare parsed documents, not bytes | — (repository convention) |
| Strict v1 validation | A hand-written validator over document-order JSON, with zod's `path: message` reason, not serde `deny_unknown_fields` | `readGateFile` returns the document as parsed so keys keep their position; `version: 1.0` is `1` to `JSON.parse`; fixture `read-reason` expects `entries.a` in the reason | — |
| Lock and atomic write | The TypeScript protocol: exclusive `<file>.lock` holding `"<pid> <uuid>\n"`, broken when the pid is gone or the lock is older than 2 s, unlocked write after 5 s, release only with our token. The write goes to a 0600 temp file in the same directory (`tempfile`), is fsynced, and is persisted over the target | Issue scope; `state-io.ts:133-167`; `fs4` advisory locks do not exclude TypeScript writers | — (fixed by the issue) |
| Telemetry API | A closed Rust enum, one variant per event with exactly its extras, so a command line or payload cannot be passed at all; a strict parser for persisted lines | `TELEMETRY_EXTRAS`; fixture `telemetry-extras` | — |

## Rejected alternatives

- **Discovery only in `toolu-state`:** `Roots::project_root` would keep spawning git on every hook outside Claude, the cost the issue was opened for.
- **An injected toplevel resolver in `Roots`:** indirection whose only implementation would be the same walk.
- **Reading ancestry and the index from files, or adding `gix`:** a large dependency or a large reimplementation for two queries that run once per session.
- **Spawning `git rev-parse` as the fallback for every unclear case silently:** kept, but limited to the named cases and documented; any other answer comes from the walk.
- **An environment-gated `#[test]` writer, or writer threads:** the first passes trivially when run alone; the second is not a separate process.
- **`fs4` advisory locks:** TypeScript writers would not see them.
- **serde `deny_unknown_fields` for the gate file:** loses key order and rejects `1.0` where TypeScript accepts it.
