# toolu-engine: registry runner and dispatch — brainstorm (#418)

**Date:** 2026-10-07   **Mode:** Delivery, Full path (a core crate's public interface, an external-process contract third-party plugins rely on, and the enforcement path every tool call takes)

## Capsule

- **Outcome:** `crates/core/engine` runs the PreToolUse and PostToolUse walks the way `@toolu/core/dispatch` and `@toolu/core/registry` do. Built-in gates run in table order, then `<config>/toolu/{pre,post}-tools.d/` in byte order. A `<spec>__<name>.json` manifest enables a compiled-in `Rule` whose matcher fits the tool. A `.sh` module keeps its byte-for-byte contract, adding a timeout and isolation. A `.js` module runs through a temporary Bun bridge. Deny wins over ask, and ask over advisory, before a tool; block wins over advisory after it. A multi-file patch is walked once per path. Shadowing, un-namespaced rejection, installed-plugin gating and the Codex prune are ported. `toolu hook pre-tools` and `toolu hook post-tools` run the engine with an empty built-in table that #419–#423 fill.
- **Material defaults / non-goals:** no gate is ported (#419–#423), `hooks.json` keeps the Bun bundles, and `fixtures/rust-ported.json` stays empty. No host calls the native entries until #425. A new shared golden, `fixtures/dispatch/cases.json`, is captured once from the TypeScript dispatcher. A Bun test and a Rust test both reproduce it. Command detection (`is_git_push`, `is_git_commit`, `push_target_root`, `push_target_branch`) moves to the engine on top of `toolu-shell`.
- **Repository evidence:** `packages/toolu-core/src/dispatch/*.ts` (548 lines), `packages/toolu-core/src/registry/{registry-run,registry-list,registry-gate,registry-paths,registry-prune,registry-types}.ts`, `packages/toolu-core/src/detect/detect-git.ts`, the TypeScript tests `dispatch-native.test.ts` and `registry-run.test.ts`, and these Rust pieces from earlier issues:
  - `toolu_runtime::registry` (`parse_name`, `ModuleManifest`, `read_manifest`, the `Rule` trait), `toolu_runtime::process::run` (process group, deadline, stdin, output budget) and `toolu_runtime::host::snapshot::codex_plugin_installed`;
  - `toolu_state::edit_records`;
  - `toolu_protocol::{decision, encode, hook::Reply::Raw}`;
  - `toolu_shell::git::{runs_git_subcommand, push_targets}`.

  The #413 and #415 goldens (`host/encode.json`, `state/gate-bytes.json`) are the precedent for a TypeScript-captured shared fixture.
- **Risk:** byte parity of merged output, since it follows `jq -n` pretty printing and TypeScript key order. `.sh` timeouts and Bun-bridge failures are new behaviour, because TypeScript has no deadline. The window between #425 and #426–#429 runs four `.js` quality modules through the bridge on every edit.
- **Handoff:** spec.

## Axes and decisions

| Axis | Decision | Evidence | Jev |
|---|---|---|---|
| CLI wiring | Wire `toolu hook pre-tools` and `toolu hook post-tools` to the engine with an empty built-in table. Keep `hooks.json` on Bun and `rust-ported.json` empty | #419 runs its cases "against `toolu hook pre-tools`", and #419, #420, #422 and #423 run in parallel. One seam, filled by appending to a table, avoids four PRs racing to wire the same entry. The #418 acceptance "checked by the benchmark" needs a binary to measure. Enforcement is unchanged at merge because no `hooks.json` entry reaches the native hook | `cli_wiring`: wire 0.58 against library-only 0.42. The tie was broken by the downstream seam argument |
| Bun bridge spawning | One `bun` process per run of consecutive `.js` modules in walk order. A `.sh` module or manifest between them splits the run. The runner stops after a deny or block and prints one result line per module. A timeout or crash isolates the module in flight, and the modules after it run in a fresh process | The live `post-tools.d` holds four consecutive `.js` modules, which would be four Bun processes per edit (13 MB plus a bundle import each). Byte order and stop-after-deny stay observable through side effects | `bridge_batching`: batch 0.88 |
| Bridge runner source | An embedded JavaScript string run with `bun -e`. The request goes in on stdin and result lines come out on stdout behind a marker prefix | `include!` is banned but a string constant is not. A `.js` file under `src/` would fall outside the TypeScript gate-reach tooling. A prototype `bun -e` that reads stdin and imports a module path works on Bun 1.4.2 | — |
| `.sh` module contract | `bash <path>`, as TypeScript runs it. Stdin is the payload plus `\n`, and the environment adds `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and `TOOLU_EDIT_*`. The status is the exit code, or 128 plus the signal number. Trailing newlines are stripped from stdout. A per-module deadline (30 s by default) is new: past it the process group is killed and the module is reported as `exited 124`, never as a deny | `dispatch-bash.ts`, `dispatch-walk.ts:90-103`. The issue asks for "a timeout and failure isolation" reported "the same way TypeScript does" | — |
| Which files are modules | `.js`, `.sh` and `.json`, as `toolu_runtime::registry::parse_name` (#414) reads them. Other executables are not admitted until #440 defines the executable protocol | TypeScript admits only `.js` and `.sh`, and today's third-party modules are `.sh` | — |
| Shadowing | A `.sh` module is shadowed when any `.js` module in the same directory has its spec, exactly as `registry-run.ts` does it. The issue's "same-name" wording is read through TypeScript's behaviour | `registry-run.ts:138` (`esmSpecs.has(entry.spec)`) | — |
| Bun missing | Before a tool: each applicable `.js` module is a deny that names the module and the Bun install. After a tool, which is context-only: the modules are skipped and one `systemMessage` advisory is shown per session, recorded by a marker under the project state dir | Issue scope. `is_enforcing` holds only for `PreToolUse` and `PermissionRequest` | — |
| Manifest problems | An unreadable, mismatched or other-version manifest gives one `toolu-registry:` stderr line, and its rule is skipped. It is never an error | Issue scope ("one advisory line, not an error"). `read_manifest` is strict (#414) | — |
| Parity oracle | `fixtures/dispatch/cases.json`, captured once from TypeScript, with exact stdout, stderr and exit code. It holds static built-ins (a decision or a throw), `.sh` bodies, `.js` sources, payloads and the installed plugins. Manifest and timeout cases are Rust-only, because TypeScript has neither | The #413, #414 and #415 golden precedent. `fixtures/README.md` says "no fixture generator in the test path" | — (repository convention) |

## Rejected alternatives

- **Library only:** the first gate PR would have to wire the CLI while three others race it, and the benchmark acceptance would have no binary to measure.
- **Listing the entries in `rust-ported.json` now:** CI would run every pre-tool gate suite against a binary with no gates.
- **One Bun process per `.js` module:** this quadruples Bun's cost per edit until #426–#429 land.
- **A persistent request/response Bun process per hook call:** it needs streaming pipes that `toolu_runtime::process` does not offer, all for a component #440 deletes.
- **Treating a timed-out `.sh` exit status as its verdict:** a module that traps SIGTERM and exits 2 would turn a timeout into a deny.
- **Running other executables now:** this changes which files TypeScript admits while both dispatchers share one registry.
