# `toolu plugins`: the installer in Rust — Design

**Date:** 2026-10-12   **Status:** Approved   **Author:** epic worker (Claude Opus 5.5)   **Topic:** #438: port `tools/toolu-cli` to `crates/plugins` as `toolu plugins`, behind a thin `@toolu/plugins` wrapper

## Problem

Plugins are installed with `npx @toolu/plugins install`, a 3,000-line Node
bundle (`tools/toolu-cli`) that #440 must remove with the rest of the
TypeScript. Its global bin is `toolu`, the same name as the native binary, so
a global npm install shadows the native CLI on `PATH`; SessionStart and
`toolu doctor` already have to reject it. The `toolu plugins` namespace is a
`planned` placeholder (`crates/cli/src/registry.rs`).

## Non-Goals

1. Writing `.toolu/plugins.json`. The TS CLI never wrote it (`writeSelection`
   is unused); `--config` only reads it.
2. The live install progress bar. Plugin crates never write stdout or stderr
   (rule 14) and a verb returns one `Outcome`; the final report is unchanged.
3. Cursor and Hermes as installer targets (`--host cursor|hermes` is refused).
4. The wrapper downloading or installing the native binary.
5. Removing the rest of the TypeScript (#440), or the OpenCode adapter
   `@toolu/opencode` itself.
6. Any change to the hook protocol (`HOOK_PROTOCOL` stays).
7. Migrating a global install for the user. A global `npm install -g @toolu/plugins`
   loses its `toolu` command; `docs/cli/installer.md` tells such users to
   reinstall (the command is now `toolu-plugins`) and that `toolu plugins …` is
   the native form.

**Reinterpreted acceptance.** The issue asks that `--config` "behave as in the
TypeScript CLI". The TS CLI parses `--config` and never reads it (`args.config`
has no use in `plugins/dispatch.ts`; `readSelection` has no caller outside its
tests). This port makes it do what the TS help and `docs/cli/installer.md`
promise, replaying the file on `install`, with the TS `readSelection` rules.

## Architecture

**Crate.** `crates/plugins`, package `toolu-plugins`, a plugin-layer library
that links only core (`toolu-protocol`, `toolu-runtime`) plus `clap`,
`serde`, `serde_json` and `jsonc-parser` 0.34 (`cst` feature; MIT, the dprint
crate the issue names). It exports `command() -> clap::Command` and
`run(&ArgMatches, &Ctx) -> Outcome`. `crates/cli/src/registry.rs` replaces the
`PLUGINS` placeholder with them; the owner stays `BUILTIN` (no plugin
directory, so no hidden `hook` verb). Modules mirror the TS tree one to one so
the port is reviewable against it:

| TS (`tools/toolu-cli/src`) | Rust (`crates/plugins/src`) |
|---|---|
| `args/`, `cli.ts` | `args.rs` (clap tree, scope rules), `lib.rs` (`run`) |
| `catalog/` | `catalog.rs` (embedded + `--catalog`), `catalog/order.rs` |
| `config/` | `replay.rs` (`--config`) |
| `host/` | `host.rs` (detection, PATH probe), `host/claude.rs`, `host/codex.rs` |
| `plugins/` | `verbs/{install,list,remove,update}.rs`, `dispatch.rs` |
| `ui/report.ts`, `ui/prompts.ts` | `report.rs`, `prompt.rs` |
| `opencode/` | `opencode/{paths,jsonc,entries,selection,state,install,remove,update,list}.rs` |

Host commands run through `toolu_runtime::process::run_with` (no shell, its
own process group, a 600 s deadline per command), arming the process guard from
`on_spawn`, so Ctrl-C kills the host command's group and `toolu` ends by the
same signal (the shell still sees 130); environment comes from
`toolu_runtime::env::Env::process()`; writes use the same temp-then-rename as
the TS `writeAtomic` (through a symlink to its target, keeping mode bits).

**JSONC.** `jsonc-parser`'s parser gives the tree with ranges and its scanner
the tokens (a comment is one token). The TS helpers `appendJsonc` and
`removeJsonc` are ported over those offsets byte for byte; the `modify`
fallbacks (add a `plugin` key, remove the key, replace a spec string, insert
into an inline array, remove an inline element) use the CST
(`CstRootNode::parse`, `object_value`, `array_value_or_set`, `append`,
`remove`, `set_raw_value`). The TS fixtures are the contract.

**Versions.** The `@toolu/opencode@<version>` target and every version the
verbs print come from `CARGO_PKG_VERSION`, the lockstep workspace version that
equals the TS `package.json` version.

**Catalog.** The binary embeds `.claude-plugin/marketplace.json`
(`include_str!`, as `crates/pr-babysit` embeds its brief). `--catalog <path>`
(a global flag of the namespace) reads another; the wrapper passes its bundled
`assets/marketplace.json`. Both are copies of the same repository file at
build time, so a released wrapper and binary of one version agree; a wrapper
newer than the binary installs the wrapper's catalog, which is why the wrapper
passes it. The embed ties the crate to the repository layout, which
`crates/plugins/README.md` says.

**Prompts.** Interactive means: no `--no-input`, no `--json`, stdin is a
terminal and `/dev/tty` opens. Prompts are line-based, written to and read from
`/dev/tty`, so stdout stays data. The logic is generic over `BufRead` + `Write`.
Multi-select lists numbered options, all selected: Enter accepts them, `1 3` or
`1,3` picks those, `q` or end of input cancels. Single-select: Enter picks the
first, a number picks that one. When stdin is a terminal but `/dev/tty` does
not open, the run is non-interactive (as the TS CLI was without a TTY): an
ambiguous host exits 64, and `install` with no names installs the whole
catalog.

**Wrapper.** `tools/toolu-cli/src/cli.ts` becomes a dependency-free Node script
built to `npm/dist/cli.js`; `npm/package.json` declares
`"bin": {"toolu-plugins": "dist/cli.js"}` and no `toolu`. It resolves the
native binary in the launcher's order (a non-empty `TOOLU_BIN` alone; else `/opt/homebrew/bin/toolu`,
`/usr/local/bin/toolu`, `/home/linuxbrew/.linuxbrew/bin/toolu`,
`$HOME/.local/bin/toolu`; then each `toolu` on `PATH`). It skips a candidate
that is not an executable regular file, whose real path is the wrapper's own
or inside an `@toolu/plugins` package, or whose `--hook-protocol` (20 s) does
not print an integer. A native binary that answers but whose
`toolu plugins install --help` fails predates this namespace: the wrapper exits
69 naming its path and the upgrade commands. Then it runs
`toolu plugins <argv…>` with `--catalog <assets/marketplace.json>` after the
verb, inherited stdio and forwarded SIGINT/SIGTERM/SIGHUP, and exits with its
status (128 plus the signal's number when a signal ends it). `--version` prints the wrapper's version.
Missing binary: exit 69 and the #457 commands. The rest of
`tools/toolu-cli/src` is deleted, with the `@clack/prompts`, `jsonc-parser`,
`zod` and `@toolu/opencode` dependencies.

## Interfaces / Schema

```text
toolu plugins [--catalog <path>] <verb>
  install [NAME...] [--scope user|project|local] [--config <path>] [-y|--yes] [--dry-run] [--no-input]
  list
  remove NAME... [--scope …] [-y|--yes] [--dry-run] [--no-input]
  update [NAME...] [--scope …] [--dry-run] [--no-input]
global: --host claude|codex|opencode (cursor, hermes → 64), --json
```

`--config` is an `install` flag only (clap refuses it elsewhere, 64), and
`--config` with names is a usage error. `--host` wins over the file's `host`.

`--scope`: claude takes all three, opencode `user` (global) or `project`, codex
none — checked once the host is known. `--config <path>` (install only, not
with names): a `.toolu/plugins.json` (`{"version":1,"generatorVersion"?,"host"?,"enabled":[…]}`);
its `enabled` is the request, its `host` applies when `--host` is absent.

Exits (#442): 0 success or already satisfied; 1 any failed step, a cancelled
prompt (`toolu: cancelled`), an unreadable `--catalog` file, an OpenCode config
or selection file that is unreadable, not JSONC or malformed; 64 usage errors
and missing input (unknown verb/flag/plugin/host, bad scope, `remove` without
names or `--yes`, ambiguous host without a prompt, no host on `PATH`, both
OpenCode scopes configured without `--scope`, bad `TOOLU_OPENCODE_PACKAGE`, a
`--catalog` file that is not JSON or not a catalog, any bad `--config` file:
missing, not JSON, wrong version, newer major, malformed). Text output is the TS report byte for byte, with every
`npx @toolu/plugins …` hint rewritten to `toolu plugins …`. Diagnostics start
`toolu: `.

`--json` (one document on stdout):

```json
list:    [{"name":"toolu","installed":true,"version":"7.11.0","enabled":true}, …]
install: {"dryRun":false,"hosts":[{"host":"claude","steps":[{"name":"toolu","outcome":"installed","detail":"installed","argv":["claude","plugin","install","toolu@toolu"]}]}]}
remove:  {"host":"opencode","steps":[{"name":"jev","removed":true,"detail":"…","argv":[],"plan":"disable jev in …"}]}
update:  {"host":"claude","steps":[{"name":"jev","outcome":"current","detail":"current at 7.11.0","argv":[…]}]}
```

`version` is omitted when absent and `plan` only on file-editing hosts, as the
TS `JSON.stringify` did.

## Failure modes and edge cases

- Host CLI missing or failing: the step is `failed` with the first stderr line
  (`install failed` when empty); a host command past its 600 s deadline is
  `failed` with `timed out after 600 s`; a failed `toolu` skips its dependents
  (`skipped: toolu failed`); other plugins continue; exit 1.
- `plugin list --json` exits non-zero or prints the wrong shape: treated as
  nothing installed (install/update), as the TS did; parse errors never
  surface as success with no steps (`update` with nothing installed exits 1).
- No host on `PATH`: 64, naming `claude, codex, opencode`. Several and no
  prompt: 64 naming the candidates. A cancelled prompt: 1.
- OpenCode config that is not JSONC, a non-array `plugin`, an invalid selection
  file: exit 1 naming the file, nothing written. Absent files are absent, not
  errors. Several writes are planned before any happens; a write failure exits
  1 naming the path.
- Concurrent edits: before writing, every planned path is read again and
  compared with the text the run read; if any changed (another `toolu plugins`
  run, an editor save), nothing is written and the run exits 1 naming the path
  (`<path> changed while toolu ran; nothing was written`). The window left is
  the rename itself.
- An empty project `plugin: []`, shadowed global files, both scopes configured:
  the TS notes and refusals, unchanged.
- `--config`: missing file, bad JSON, wrong or missing `version`, a
  `generatorVersion` with a newer major than the binary, a malformed `enabled`:
  64 naming the file. An empty `enabled` installs nothing and exits 0.
- Unknown plugin names: 64 before any host command runs. A dependency cycle in
  a `--catalog` file: 64.
- Wrapper: a `toolu` on `PATH` that is the wrapper (symlink, copy inside the
  package) or a non-native script is skipped; when the native binary dies by a
  signal the wrapper re-raises that signal on itself, and only a status it
  cannot read maps to 1.

## Acceptance criteria

- **AC-1:** `toolu plugins --help` lists `install`, `list`, `remove`, `update`
  and no `planned`; `docs/cli/plugins.md` and `docs/cli/commands.json` are
  regenerated from the binary and `cargo xtask gate --only docs-cli --only cli-compat` passes.
- **AC-2:** Given a global `opencode.jsonc` with line and block comments, a
  trailing comma and an existing `plugin` array, `toolu plugins install ts-quality --host opencode`
  appends `@toolu/opencode@<version>` on its own line and every byte outside
  that line is unchanged; the TS JSONC fixtures (`appendJsonc`, `removeJsonc`,
  tuple replace, block comments) give the same bytes in Rust unit tests.
- **AC-3:** With a `claude` CLI on `PATH` whose `plugin install toolu@toolu`
  fails, `toolu plugins install toolu ts-quality jev --host claude` reports
  `toolu` failed, `ts-quality` `skipped: toolu failed`, `jev` installed, and
  exits 1.
- **AC-4:** `--scope local --host opencode` and `--scope user --host codex` exit
  64; `--config <file>` with `{"version":1,"enabled":["rust-quality"]}`
  dry-runs `toolu` then `rust-quality`; a file with `generatorVersion` of a newer
  major exits 64; `--no-input` with `claude` and `codex` on `PATH` exits 64
  naming both.
- **AC-5:** The OpenCode subprocess suite of `tools/toolu-cli` (verbs, scope,
  notes, selection, selection parity with `@toolu/opencode/select`, config
  read/write through symlinks and modes) passes against the built Rust binary.
- **AC-6:** `bun run smoke:opencode-entry cli.install cli.lifecycle` passes on
  the pinned OpenCode, driving the npm wrapper, which runs the built binary.
- **AC-7:** `tools/toolu-cli/npm/package.json` declares `toolu-plugins` and no
  `toolu` bin; with a `toolu` on `PATH` that links to the wrapper and no native
  binary, the wrapper exits 69 naming `curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash`
  and `brew install falconiere/tap/toolu`; with the native binary installed,
  `install --dry-run --host claude` works from inside this repository and from
  an empty directory; the packed tarball holds `dist/cli.js` and
  `assets/marketplace.json`.
- **AC-8:** No skill, command, agent or hook text of `plugins/toolu` (and the
  native session notices and uninstall hint) names `npx @toolu/plugins`; they
  name `toolu plugins …`, and `cargo xtask check-markdown-cli` passes.
- **AC-9:** `--json` on each verb prints exactly one JSON document of the shape
  above; `--host cursor` exits 64 naming the supported hosts.
- **AC-10:** `cargo xtask gate` passes with `crates/plugins` at the quality bar
  (no exemption, ≥85% line coverage).
- **AC-11:** The prompt logic, over in-memory readers, maps Enter to every
  option (multi) or the first (single), `1 3` and `1,3` to those options, an
  out-of-range or non-numeric answer to a re-ask, and `q` or end of input to
  `cancelled`; a run with `--no-input`, or with stdin not a terminal, never
  prompts and exits 64 on two hosts.
- **AC-12:** When a planned OpenCode file changes between the read and the
  write, the run writes nothing and exits 1 naming the file; a host command
  that outlives its deadline is a `failed` step whose detail says
  `timed out after <n> s`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | the built binary | four verbs, regenerated docs | placeholder gone | `cargo xtask gate --only docs-cli --only cli-compat`; `INSTA_UPDATE=always cargo test -p toolu-cli --test contract` then rerun |
| AC-2 | `COMMENTED`, `strict`, `inline`, `tuple`, block-comment fixtures of `config.test.ts`, copied as string constants into `crates/plugins/src/opencode/tests/jsonc_test.rs` (the TS file is deleted in this PR, so nothing can diverge) | identical bytes | comma before a trailing comment; `]` after a block comment | `cargo test -p toolu-plugins jsonc`; bun `verbs.test.ts` lifecycle |
| AC-3 | a temp `PATH` with a `claude` script that fails only `toolu@toolu` and logs argv (fault injection at the host boundary); plus the real `claude` against a temp `CLAUDE_CONFIG_DIR` when it is on `PATH` (`tools/toolu-cli/src/__tests__/claude.test.ts`, ported from `install.test.ts`) | the three outcomes, exit 1 | core failure vs other failure | `cargo test -p toolu-cli --test plugins_hosts`; `bun test tools/toolu-cli` |
| AC-4 | temp selection files, stub `claude`/`codex` on `PATH` | exits and dry-run order | newer major, empty `enabled` | `cargo test -p toolu-cli --test plugins_args` |
| AC-5 | isolated HOME/XDG profiles and git projects; the fixture spawns `requiredBuiltTooluBinary()` (`@toolu/conformance/harness/entry-command`) with `plugins` instead of `src/cli.ts` | the TS assertions; only `EXIT.usage` (2) and `EXIT.missingInput` (3) become 64 and the `toolu: ` prefix stays | malformed JSONC, both scopes, symlink, mode 600 | `bun test tools/toolu-cli` |
| AC-6 | pinned `opencode-ai` and a packed `@toolu/opencode` | scenarios pass | commented global `opencode.jsonc` | `bun run smoke:opencode-entry cli.install cli.lifecycle` |
| AC-7 | the npm folder, a temp `PATH`, `TOOLU_BIN` set to the built binary; `--dry-run --host claude` needs no `claude` on `PATH` (a failed `plugin list` reads as nothing installed) | exit 69 / success | wrapper on `PATH` as `toolu`; a native binary without `plugins install` | `bun test tools/toolu-cli/src/__tests__/wrapper.test.ts tooling/src/__tests__/npx-invocation.test.ts`; `cargo xtask gate --only pack` |
| AC-8 | `plugins/toolu/**`, `crates/toolu/src/lifecycle/notices.rs`, `crates/core/runtime/src/host/roots.rs` | no `npx @toolu/plugins` | the wrapper's own README and `docs/cli/installer.md` keep it, correctly | `! git grep -n 'npx @toolu/[p]lugins' -- plugins/toolu crates/toolu crates/core`; `cargo xtask check-markdown-cli` |
| AC-9 | each verb with `--json`; a `claude` stub that fails `toolu@toolu` | parses as one document, exit 1 on the failing run | failure keeps one document | `cargo test -p toolu-cli --test plugins_args` |
| AC-10 | the workspace | green | coverage floor | `cargo xtask gate` |
| AC-11 | answers `""`, `"1 3"`, `"1,3"`, `"9"`, `"x"`, `"q"`, EOF | selections, re-ask, cancel | out of range, EOF | `cargo test -p toolu-plugins prompt`; `cargo test -p toolu-cli --test plugins_args` (no pty case: `script` differs between macOS and Linux, and the `/dev/tty` open is a few lines of glue) |
| AC-12 | a profile config rewritten by the test between `plan` and `apply` (the crate's apply function called directly in a unit test); a `sleep 5` command run with a 1 s deadline through the same step runner | exit 1 and the file holds the test's text; a `failed` step with the timeout detail | the 600 s production deadline is a parameter, not an env knob; Ctrl-C relies on `toolu_runtime::process::guard`, covered by that crate's tests | `cargo test -p toolu-plugins -- apply run_step` |

## Documentation impact

`docs/cli/installer.md` (the wrapper and `toolu plugins`), generated
`docs/cli/plugins.md` and `commands.json`, `README.md` install snippets,
`AGENTS.md` (Tech stack installer bullet, Key files rows for `tools/toolu-cli`),
`docs/install.md` (agent command shell: the wrapper no longer shadows),
`docs/opencode.md` and `docs/opencode-migration.md` where they run `src/cli.ts`
or name the `toolu` bin, `docs/testing.md`, `crates/plugins/README.md`, the
AGENTS.md CI table rows that name the installer (`opencode`, `docs`, `rust`),
`tools/toolu-cli/npm/README.md`, the `tools/toolu-cli/package.json` `build`
script and `tsconfig.json`, `tooling/src/opencode-host/scenarios-cli.ts` and
`tooling/src/__tests__/npx-invocation.test.ts` comments, and the generated
copies under `tools/toolu-opencode/generated/resources/repo/docs`.

**Replaced TS unit suites.** `args/parse` → `src/tests/args_test.rs`;
`catalog/{manifest,order}` → `src/tests/catalog_test.rs`, `src/catalog/tests/order_test.rs`;
`config/selection` → `src/tests/replay_test.rs`; `host/adapters` →
`src/host/tests/{claude,codex}_test.rs`, `src/tests/host_test.rs`;
`plugins/{install,update,dispatch}` → `src/verbs/tests/*_test.rs` and
`crates/cli/tests/plugins_{hosts,args}.rs`; `ui/{report,prompts}` →
`src/tests/{report,prompt}_test.rs`; `ui/progress` is dropped with the bar;
`opencode/{config,selection,notes}` (the pure parts) →
`src/opencode/tests/*_test.rs`. Every module file with a function gets its
co-located test (rule 7).

## Open Questions

None blocking. Decided in the brainstorm: delete the TS installer, contract
exit codes, drop the progress bar, `feat(plugins)!`, `--config` reads only and
replays on `install` (the reinterpreted acceptance above). The TS suites assert
no cancel or 130 path through the binary (`prompts.test.ts` checks the clack
sentinel only, which goes with clack). The real-`claude` Bun case reports a
visible `test.skipIf` skip where `claude` is absent.
