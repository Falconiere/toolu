# Native hook launcher — Design

**Date:** 2026-10-05   **Status:** Approved   **Author:** Claude Code   **Topic:** Issue #412, find the installed `toolu`, fail closed without it, apply the #411 skew rule

## Problem

Epic #402 moves every hook to `toolu hook …` on one installed native binary (#411). A host runs a `hooks.json` command string, so that string decides what happens when the binary is:

- missing;
- shadowed by the npm wrapper of the same name;
- crashed;
- older or newer than the plugin.

Today's launcher (`packages/toolu-core/src/launcher/launcher.ts`) only knows Bun. Without a generated, gated native launcher, the first ported hook (#418 onward) has no safe way to run, and #425 cannot switch the toolu plugin.

## Non-Goals

1. Switching any committed `hooks.json` entry to the native launcher. #425 does that once the hooks are ported. Until then no entry is native, and `check-hooks` gates zero native entries plus every manifest.
2. The CLI contract: clap, namespaces, `toolu commands --json`, the exit code table (#442). The binary here parses argv by hand, and #442 replaces the parser while keeping the argv below.
3. Porting any hook body. The only native hook is toolu's `session-start` diagnostic line; #424 grows it into the full session-start port.
4. The binary's internal deadline below the entry `timeout`. That belongs to the hook main wrapper with `catch_unwind` (#413). Here the launcher maps a crashed or killed binary to exit 2, and every generated entry carries a `timeout`. Until #413, a hook that hangs past the host `timeout` is a non-blocking host error; `docs/install.md` states this gap.
5. Real installs through the Homebrew formula and the installer script. Those channels are #456 and #457. The PR body names `cargo xtask launcher-e2e` as the check for #457 to run against real installs; this issue edits no other issue. Here CI copies the release binary into the same directories (see AC-4). This is a deliberate narrowing of #412's first acceptance item: the channels do not exist yet.
6. `toolu doctor` (#445), Windows execution (#411 defers it; `commandWindows` is generated, not run), and the `TOOLU_RUNTIME=bun` escape hatch (#425).
7. A parity check between the TypeScript and Rust copies of the enforcing-event set and the install commands. Both are small, and #440 deletes the TypeScript side.

## Architecture

Four pieces, placed by the layer table and capability owners (`tooling/conventions/guardrails/rust/{layers,rules}.json`):

- **`toolu-protocol`** (`crates/core/protocol`): `HOOK_PROTOCOL: u32 = 1`, the enforcing-event set (`PreToolUse`, `PermissionRequest`, as in TypeScript), the two install commands, and the launcher generator `launcher::hook(&Target) -> Result<LauncherHook, String>`. It also owns `read_stdin()`, because the crate owns hook stdio. Otherwise these are pure functions.
- **`toolu-runtime`** (`crates/core/runtime`):
  - `manifest::read(plugin_root)` reads `version` and `hookProtocol`.
  - `skew::assess` applies #411.
  - `install::upgrade_command(exe)` picks `brew upgrade toolu` or the installer command.
  - `invocation::{args, current_exe}` wraps `std::env`, which only `toolu-runtime` may read (rule 14).
- **`toolu-cli`** (`crates/cli`, package `toolu-cli`, bin `toolu`) is the only binary. Its argv:
  - `toolu --version`;
  - `toolu --hook-protocol`;
  - `toolu [<plugin>] hook <name> [--event <Event>] [--plugin-root <dir>]`.

  It runs the skew prelude and then the named native hook. `output` is its only stdout and stderr writer. It is added to `Cargo.toml` `members` and `folders.json` `crates`.
- **`xtask`** depends on `toolu-protocol` (tooling may use core). It gains three tasks:
  - `print-hook` prints an entry;
  - `check-hooks` gates entries and manifests, and runs as a new `cargo xtask gate` step `hooks`;
  - `launcher-e2e` runs the real launcher against an installed binary.

  Each task has a passing and a failing test in `inventory.json`.

The argv follows #409's seam: toolu's hooks are `toolu hook <name>`, and another plugin's are `toolu <plugin> hook <name>`. Jev decided the remaining forks in brainstorm (`docs/toolu/brainstorms/2026-10-05-native-hook-launcher.md`):

- The event and the plugin root travel as argv, because Claude substitutes `${CLAUDE_PLUGIN_ROOT}` as text and does not always export it.
- The semver advisory prints at SessionStart only.
- End to end at the real install directories runs in CI.

**Decisive trade-off.** One extra short spawn (`--hook-protocol`) per hook call buys skipping the npm wrapper `toolu` (`tools/toolu-cli/npm/package.json` declares that bin). #410's budget measures the whole launcher, `sh` included.

**Dependencies.** `serde` and `serde_json` come from `[workspace.dependencies]`; nothing new reaches cargo-deny.

**Reuse.** The transition fallback copies today's Bun chain byte for byte. Today's message rules also carry over: one line, and no character that sh single quotes, cmd `echo` or JSON reinterpret.

`tooling/src/check-hooks-json.ts` tests for the native marker (`--hook-protocol` in `command`) before `isLauncherHook`, because a native command also names `hooks/dist/` in its fallback. It skips native entries and still gates every Bun entry.

## Interfaces / Schema

**Names.** `plugin` and `name` must match `^[a-z0-9]+(-[a-z0-9]+)*$`, and `Event` must match `^[A-Z][A-Za-z]+$`. They are spliced into sh and cmd, so the generator returns `Err` otherwise. `<argv>` is `hook <name>` when the plugin is `toolu`, and `<plugin> hook <name>` otherwise.

**POSIX `command`.** The generator joins these lines with `; `. `<run>` is `<argv> --event <Event> --plugin-root "${CLAUDE_PLUGIN_ROOT}"`, and `<bundle>` is `"${CLAUDE_PLUGIN_ROOT}/hooks/dist/<name>.js"`.

```sh
t=
if [ -n "$TOOLU_BIN" ]; then set -- "$TOOLU_BIN"; else set -- /opt/homebrew/bin/toolu /usr/local/bin/toolu /home/linuxbrew/.linuxbrew/bin/toolu "$HOME/.local/bin/toolu" "$(command -v toolu 2>/dev/null)"; fi
for c in "$@"; do if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then case $("$c" --hook-protocol 2>/dev/null </dev/null) in ''|*[!0-9]*) ;; *) t=$c; break;; esac; fi; done
# enforcing event:
if [ -n "$t" ]; then "$t" <run>; s=$?; case $s in 0|2) exit $s;; esac; printf '%s\n' "blocked: <plugin> plugin: toolu ended with status $s, so the action is blocked. See docs/install.md." >&2; exit 2; fi
# context event:
if [ -n "$t" ]; then exec "$t" <run>; fi
# transition fallback, removed by #440:
if [ -z "$TOOLU_BIN" ]; then b=; for c in "$TOOLU_BUN" "$(command -v bun 2>/dev/null)" "$HOME/.bun/bin/bun"; do if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then b=$c; break; fi; done; if [ -n "$b" ] && [ -f <bundle> ]; then printf '%s\n' '<fallback advisory>' >&2; exec "$b" <bundle>; fi; fi
# missing, enforcing:
printf '%s\n' 'blocked: <missing message>' >&2; exit 2
# missing, context:
printf '%s\n' '{"systemMessage":"<missing message>"}'; exit 0
```

The string has no version input, so it is byte-identical across releases. Golden strings for one enforcing and one context entry are committed as `crates/core/protocol/src/tests/fixtures/launcher-{pre-tool-use,session-start}.txt` and compared byte for byte.

**Messages.** These are cmd-safe: no `( ) & < > ^ % "` and no `'`. The only exception is `|`, which is written as `^|` inside `commandWindows`.

- **Missing:** `<plugin> plugin: toolu is not installed - checked TOOLU_BIN, /opt/homebrew/bin, /usr/local/bin, /home/linuxbrew/.linuxbrew/bin, ~/.local/bin and PATH, and a TOOLU_BIN that is not a native toolu is never replaced. Install it with: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash, or: brew install falconiere/tap/toolu. Then restart the session. See docs/install.md.`
- **Fallback advisory** (stderr): `<plugin> plugin: native toolu not found, running the Bun bundle for this transition release - see #425. Install toolu with: <installer> or: <brew>.`

**`commandWindows`.** Today's Bun chain for `hooks\dist\<name>.js`, with the missing message above. No native binary is resolved on Windows (#411).

**Entry JSON** (`print-hook`): `{"type":"command","command":…,"commandWindows":…,"timeout":60}`. `timeout` is in seconds on both Claude Code and Codex.

**xtask tasks.** Each exits 0 when clean, 1 on findings and 2 on usage or setup errors, as xtask already does.

- **`cargo xtask print-hook <plugin> <Event> <name> [--timeout N] [--root DIR]`.** `N` is an integer in 1..=600; `0`, `601` and non-numeric values exit 2. A name that fails the regexes, or a missing `<root>/plugins/<plugin>` directory, also exits 2.
- **`cargo xtask check-hooks [--root DIR]`.** It walks `plugins/*/hooks/hooks.json`. A hook is native when its `command` contains `--hook-protocol`. For each native hook, the plugin comes from the directory, `Event` from the `hooks.json` key, and `name` from the word after the first ` hook ` in `command`. No generated message contains ` hook `. A native hook whose name cannot be parsed is a finding that names the file and the entry. The check then requires:
  - `type == "command"`;
  - `command` and `commandWindows` equal the generator's output;
  - `timeout` is an integer in 1..=600.

  Each finding names the file, `<Event>[i].hooks[j]` and the expected string. Every `plugins/*/.claude-plugin/plugin.json` and `.codex-plugin/plugin.json` must have an integer `hookProtocol` equal to `HOOK_PROTOCOL`.
- **`cargo xtask launcher-e2e --bin <path>`.** The setup must hold, or the task exits 2:
  - `<path>` is an executable `toolu` directly inside one of the four fixed install directories;
  - no earlier fixed directory holds a `toolu`.

  It then generates the toolu `SessionStart` and `PreToolUse` launchers for `session-start` and runs each with `sh -c` under `env -i HOME=$HOME PATH=/usr/bin:/bin CLAUDE_PLUGIN_ROOT=<temp>`. The temp plugin root holds a manifest made from `toolu --version` and `toolu --hook-protocol`. Findings:
  - SessionStart with `{"source":"startup"}` must exit 0 with `toolu runtime: native <version> at <path>`, the path canonicalized on both sides;
  - PreToolUse with the manifest's `hookProtocol` raised by 1 must exit 2 with `blocked:`.

**Binary.**

- `toolu --version` prints `toolu <CARGO_PKG_VERSION>`.
- `toolu --hook-protocol` prints `1`.
- Any argv other than the three forms exits 64 with usage on stderr.
- Hook stdin is read whole by `protocol::read_stdin`.

**Manifest.** `manifest::read(root)` reads `<root>/.claude-plugin/plugin.json`. If that file does not exist, it reads `<root>/.codex-plugin/plugin.json`. An existing but unreadable or malformed `.claude-plugin` file is an error, never a fallback. `hookProtocol` must be a JSON integer in 1..=`u32::MAX`; `"1"`, `1.0`, `0`, negative or absent values are errors. Every `plugins/*/.claude-plugin/plugin.json` and `.codex-plugin/plugin.json` gains `"hookProtocol": 1`.

**Version compare.** The project's own parser takes `MAJOR.MINOR.PATCH` of decimal integers. A `-prerelease` or `+build` suffix is stripped before the numeric compare, and no `semver` crate is added. Equal triples with different full strings count as "differs". An unparsable version gives "cannot compare".

**Skew** (`p` = the plugin manifest's version and `P` its `hookProtocol`; `b` = the binary's version and `n` = `HOOK_PROTOCOL`). The `P < n` row is unreachable with real data while `n` is 1, because `P` < 1 is a bad manifest; it is covered by `skew::assess` unit tests over integers (`n` = 2, `P` = 1) until protocol 2 exists:

| Case | Enforcing event | Context event |
|---|---|---|
| Same protocol, same version | runs the hook | runs the hook |
| Same protocol, `b` older (major included) | runs | runs; at SessionStart one advisory line before the hook output: `toolu <b> is older than the <plugin> plugin <p>; upgrade it: <upgrade>` |
| Same protocol, `b` newer | runs | runs; at SessionStart: `toolu <b> is newer than the <plugin> plugin <p>; update the plugins from your host's marketplace` |
| Same protocol, equal numeric triples but different strings | runs | runs; at SessionStart: `toolu <b> does not match the <plugin> plugin <p>; update the plugins from your host's marketplace` |
| Same protocol, a version that does not parse | runs | runs; at SessionStart: `toolu <b> cannot be compared with the <plugin> plugin <p>; update the plugins from your host's marketplace` |
| `P` > `n` | exit 2, stderr `blocked: <plugin> plugin: hook protocol <P> needs a newer toolu - toolu <b> speaks protocol <n>; upgrade it: <upgrade>` | exit 0, `{"systemMessage": same text without "blocked: "}`; the hook does not run |
| `P` < `n` | exit 2, stderr `blocked: <plugin> plugin: hook protocol <P> is older than toolu <b> - protocol <n>; update the plugins from your host's marketplace` | same text as a `systemMessage`, exit 0; the hook does not run |
| Manifest absent, unreadable, malformed, bad `hookProtocol`, or `--plugin-root ""` | exit 2, stderr `blocked: <plugin> plugin: cannot read hookProtocol from <root>: <reason>; update the plugins from your host's marketplace` | the same text as a `systemMessage`, exit 0; the hook does not run |

`<upgrade>` comes from `install::upgrade_command(current_exe)`, which canonicalizes the path first. If a component is `Cellar`, or the path starts with `/opt/homebrew/` or `/home/linuxbrew/.linuxbrew/`, it is `brew upgrade toolu`. Otherwise it is `curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash`. `/usr/local/bin` alone never implies Homebrew; an Intel Homebrew link there resolves into `/usr/local/Cellar`.

**Output composition.** The prelude owns the advisory and the mismatch output. A hook returns an outcome: an exit code, an optional `systemMessage`, and optional stderr. The cli writes one stdout JSON whose `systemMessage` is the advisory line, a newline, then the hook's message, either part omitted when empty. Nothing is written when both are empty. A mismatch never reaches the hook.

**Native hooks.**

- **toolu `session-start`.** It parses stdin as JSON. When `source` is `startup` or `resume`, under any event, its message is `toolu runtime: native <version> at <canonical current_exe>`. It exits 0. Empty, non-JSON or truncated stdin, or another `source`, adds no diagnostic.
- **Any other name.** On an enforcing event it exits 2 with `blocked: <plugin> plugin: toolu <version> has no hook <name>; upgrade it: <upgrade>`. On a context event it prints the same text as a `systemMessage` and exits 0.
- **Without `--plugin-root`.** The skew prelude is skipped; this covers direct runs such as the #409 harness. An empty value is not absent (see the table).
- **Without `--event`.** The run counts as enforcing.

**CI.**

- **`.github/ci-paths.json` `rust` group** gains `plugins/*/hooks/hooks.json`, `plugins/*/.claude-plugin/plugin.json` and `plugins/*/.codex-plugin/plugin.json`, so a PR that edits them runs the gate.
- **`tests.yml` `rust` job** gains a step after the gate: build `--release -p toolu-cli`, then for each directory in turn run `sudo mkdir -p`, `sudo install -m 0755`, `cargo xtask launcher-e2e --bin <dir>/toolu` and `sudo rm -f`. The directories are:
  - Linux: `/home/linuxbrew/.linuxbrew/bin`, `/usr/local/bin`;
  - macOS: `/opt/homebrew/bin`, `/usr/local/bin`.
- **`rust-musl`** checks that the `toolu` binary, not `xtask`, is static.

## Failure modes and edge cases

- **Bad `TOOLU_BIN`:** missing, a directory, not executable, or failing the signature. It fails closed: enforcing events exit 2 with the missing message, and context events print it as a `systemMessage`. No install directory is searched and Bun is not used. An empty `TOOLU_BIN` counts as unset.
- **Unset `HOME`:** the candidate becomes `/.local/bin/toolu`, which is harmless, and the search continues.
- **Npm wrapper or another non-native `toolu`:** its `--hook-protocol` prints a non-integer, several lines, or nothing, so it is skipped.
- **`--hook-protocol` or the hook hangs:** the host `timeout` applies, and Claude and Codex treat that as a non-blocking error. This gap is open until #413 (Non-Goal 4).
- **Abort, segfault, SIGKILL or OOM** (statuses 134, 139, 137, …): enforcing events exit 2 with `blocked:` and the status. Context events `exec`, so the host sees the raw status, which is non-blocking.
- **Spaces** in `CLAUDE_PLUGIN_ROOT`, `HOME` or `TOOLU_BIN`: always quoted.
- **Stdin:** the probe reads `/dev/null`, so the payload reaches the hook untouched.
- **Skew inputs:** malformed manifests, bad `hookProtocol`, empty `--plugin-root` and unparsable versions behave as the skew table says. A `.claude-plugin/plugin.json` that is malformed does not fall back to `.codex-plugin`.
- **A real `toolu` already in a fixed install directory:** the `crates/cli` launcher tests and the `xtask` `launcher_e2e` tests check this first and fail with the path. They never skip. A developer with toolu installed runs them in a container, or with the binary moved aside (`docs/install.md`).
- **Bun fallback with no `hooks/dist/<name>.js`:** the missing branch runs.

## Acceptance criteria

- **AC-1:** The generator output equals the committed goldens byte for byte, for `(toolu, PreToolUse, pre-tools)` and `(toolu, SessionStart, session-start)`.
  - `cargo xtask print-hook toolu PreToolUse pre-tools` prints that entry with `"timeout":60`.
  - It exits 2 for `--timeout 0`, `--timeout 601`, a name `Bad_Name`, and an unknown plugin.
  - `cargo xtask check-hooks` exits 0 on the repository. On a temp copy it exits 1 and names the file and expected string when a native entry was hand-edited, when it lacks `timeout`, and when a `plugin.json` lacks `hookProtocol` or has `2`.
- **AC-2:** A temp copy of the repository's `plugins/` tree with one native entry written from `print-hook` passes `check-hooks`. After every `plugin.json` version is bumped from 7.10.0 to 7.11.0 (and `8.0.0`), it still passes and the entry bytes are unchanged.
- **AC-3:** With no `toolu` and no Bun (`PATH=/usr/bin:/bin` with no `bun` in it, temp `HOME`):
  - the generated `PreToolUse` command exits 2, and its stderr starts `blocked:` and contains "toolu is not installed" and both install commands;
  - the `SessionStart` command exits 0 with one JSON line whose keys and value types satisfy `tooling/fixtures/codex-hook-schemas/session-start.command.output.schema.json`, checked by a Rust test helper that reads that schema;
  - the missing and fallback messages contain none of the cmd-unsafe characters except `|`, and the generated `commandWindows` writes every `|` as `^|`.
- **AC-4:** The built binary at `$HOME/.local/bin/toolu` (temp `HOME`, `PATH=/usr/bin:/bin`) runs the generated `SessionStart` command, which prints `toolu runtime: native <version> at <canonical path>`. `cargo xtask launcher-e2e --bin $HOME/.local/bin/toolu` exits 0 under a temporary `HOME` holding the built binary, with no `sudo` needed. It exits 1 when the binary there reports another path, and 2 when `--bin` is outside the fixed directories. In CI, `launcher-e2e` passes on Linux and macOS with the release binary at the Homebrew bin directory and at `/usr/local/bin`.
- **AC-5:** Skew through the generated launcher, with the real binary in `$HOME/.local/bin` and temp plugin roots, the payload carrying `"source":"startup"`:
  - **Same version:** SessionStart prints the diagnostic with no advisory, and the toolu `session-start` hook generated under PreToolUse exits 0 with the diagnostic.
  - **Plugin one minor ahead (binary older):** SessionStart prints `older than` with the installer command. With the binary reached through a symlink into `<temp>/Cellar/toolu/<v>/bin/toolu`, it says `brew upgrade toolu`.
  - **Plugin one minor behind (binary newer):** SessionStart prints `newer than` and `update the plugins`.
  - **Plugin one major ahead, same protocol:** SessionStart advises, and PreToolUse exits 0.
  - **Plugin `hookProtocol` = binary + 1:** PreToolUse exits 2 with `blocked:` and the upgrade command, and SessionStart prints the mismatch as a `systemMessage` with exit 0.
  - **Plugin `hookProtocol` `0`, `"1"` or absent:** a bad manifest. PreToolUse exits 2 with `cannot read hookProtocol`.
  - **`P` < `n`:** the text is `is older than toolu` with `update the plugins`, in a `skew::assess` unit test.
- **AC-6:** `TOOLU_BIN` set to a 0644 file makes the generated `PreToolUse` command exit 2 with `blocked:`, even with a valid binary in `$HOME/.local/bin` and Bun available.
- **AC-7:** The real npm wrapper is first on `PATH` as `toolu`. The test builds it from `tools/toolu-cli/src/cli.ts` with `bun build --target=node`, since `dist/cli.js` is not committed; it answers `toolu: unknown flag: --hook-protocol` with exit 2. With the real binary in `$HOME/.local/bin`, the SessionStart output is the native diagnostic.
- **AC-8:** Two `toolu` stand-ins pass the signature check and then `kill -ABRT $$` or `kill -SEGV $$`. Each makes the generated `PreToolUse` command exit 2 with `blocked:` and the status (134 or 139).
- **AC-9:** With no `toolu`, the real Bun on `PATH`, and a plugin root holding `hooks/dist/session-start.js`, the generated `SessionStart` command prints the fallback advisory on stderr. The bundle prints `{"systemMessage":"bundle"}` and exits 3, and both the stdout and the exit status 3 pass through.
- **AC-10:** `bun run check:hooks-json` passes on a fixture repository whose `hooks.json` holds a native entry, and fails on the same fixture with one Bun entry hand-edited.
- **AC-11:** `toolu --hook-protocol` prints `1`, `toolu --version` prints `toolu ` plus the workspace version read from `Cargo.toml` at test time, and `toolu --nope` exits 64. `cargo xtask gate` passes with `crates/cli`, the three tasks in `inventory.json` and the `hooks` step.

## Acceptance evidence

| AC | Real input → expected observation | Boundary | Runnable check |
|---|---|---|---|
| 1 | Goldens; repository `plugins/`; temp copies with edits → exit 0/1/2 with named findings | zero native entries in the real tree; range and regex errors | `cargo test -p toolu-protocol`; `cargo test -p xtask print_hook check_hooks`; `cargo xtask check-hooks` |
| 2 | Temp copy of real `plugins/` plus a native entry; versions 7.11.0 and 8.0.0 → still clean | minor and major bumps | `cargo test -p xtask check_hooks` |
| 3 | `sh -c <command>`, no toolu, no Bun → exit 2 + stderr / exit 0 + schema-valid JSON | enforcing vs context | `cargo test -p toolu-cli --test launcher_missing` |
| 4 | Built binary in `~/.local/bin`; CI: release binary in the Homebrew bin dir and `/usr/local/bin` | install dir absent from PATH; wrong path; outside dirs | `cargo test -p toolu-cli --test launcher_native`; `cargo test -p xtask launcher_e2e` (exit 0 via temp `HOME`, 1, 2); CI step |
| 5 | Temp plugin roots with edited `version` / `hookProtocol`; Cellar symlink | the skew cases listed | `cargo test -p toolu-cli --test launcher_skew` |
| 6 | `TOOLU_BIN` = 0644 file | valid binary and Bun also present | `cargo test -p toolu-cli --test launcher_resolution` |
| 7 | The real npm wrapper built from `tools/toolu-cli/src/cli.ts`, first on PATH | wrapper first on PATH | `cargo test -p toolu-cli --test launcher_resolution` |
| 8 | Stand-ins that `kill -ABRT $$` / `kill -SEGV $$` | 134 / 139 | `cargo test -p toolu-cli --test launcher_crash` |
| 9 | Real Bun, a real `.js` bundle that exits 3 | no toolu, Bun present | `cargo test -p toolu-cli --test launcher_fallback` |
| 10 | Temp repo with one native and one edited Bun entry | native skipped, Bun still gated | `bun test tooling/src/__tests__/check-hooks-json.test.ts` |
| 11 | Built binary; workspace `Cargo.toml` | unknown flag | `cargo test -p toolu-cli`; `cargo xtask gate` |

## Documentation impact

- **`docs/install.md`:** update the policy and skew sections with what is now implemented:
  - the launcher resolution order and the signature check;
  - the missing and crash messages, and the transition fallback;
  - `TOOLU_BIN`, the hang gap until #413, and how to run the launcher tests with toolu installed.
- **`docs/runtime.md`:** a "Native launcher" section pointing at install.md, plus `cargo xtask print-hook`.
- **AGENTS.md:**
  - key-files rows for `crates/cli` and the protocol launcher;
  - `print-hook`, `check-hooks` and `launcher-e2e` in the `crates/xtask` row;
  - the Contributing line for native hook entries;
  - the CI table's rust job (launcher e2e step) and the `rust` path-group list.
- **`.github/ci-paths.json`** and **`.github/workflows/tests.yml`** (CI only).

## Open Questions

None. Decided, with reasons in brainstorm or above:

- argv carries the event and the plugin root;
- the advisory prints at SessionStart only;
- the timeout defaults to 60 s, with `--timeout` per entry;
- manifest lookup order and the strict manifest rules;
- the project's own version parser;
- the Homebrew rule.

Verified 2026-10-05:

- Codex 0.160.0 installs and enables a plugin whose `.codex-plugin/plugin.json` carries `hookProtocol`. The check used an isolated `CODEX_HOME`, `codex plugin marketplace add` and then `codex plugin add toolu@toolu`, and the cache keeps both `.claude-plugin` and `.codex-plugin`.
- `claude plugin validate` passes with `hookProtocol`.
