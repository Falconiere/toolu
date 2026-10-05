# Native hook launcher — Design

**Date:** 2026-10-05   **Status:** Draft   **Author:** Claude Code   **Topic:** Issue #412, find the installed `toolu`, fail closed without it, apply the #411 skew rule

## Problem

Epic #402 moves every hook to `toolu hook …` on one installed native binary (#411). A host runs a `hooks.json` command string, so that string decides what happens when the binary is missing, shadowed by the npm wrapper of the same name, crashed, or older or newer than the plugin. Today's launcher (`packages/toolu-core/src/launcher/launcher.ts`) only knows Bun. Without a generated, gated native launcher, the first ported hook (#418 onward) has no safe way to run, and #425 cannot switch the toolu plugin.

## Non-Goals

1. Switching any committed `hooks.json` entry to the native launcher. #425 does that once the hooks are ported; until then no entry is native and `check-hooks` gates zero native entries plus every manifest.
2. The CLI contract: clap, namespaces, `toolu commands --json`, exit code table (#442). The binary here parses argv by hand and #442 replaces the parser while keeping the argv below.
3. Porting any hook body. The only native hook is toolu's `session-start` diagnostic line; #424 later grows it into the full session-start port.
4. The binary's internal deadline below the entry `timeout`. That is the hook main wrapper with `catch_unwind` (#413); here the launcher maps a killed or crashed binary to exit 2, and every generated entry carries a `timeout`.
5. Release artifacts (#456), the installer and Homebrew formula (#457), `toolu doctor` (#445), Windows execution (#411 defers it; `commandWindows` is generated, not run).
6. The `TOOLU_RUNTIME=bun` escape hatch (#425).

## Architecture

Three pieces, placed by the layer table and capability owners (`tooling/conventions/guardrails/rust/{layers,rules}.json`):

- **`toolu-protocol`** (`crates/core/protocol`): `HOOK_PROTOCOL: u32 = 1`, the enforcing-event set (`PreToolUse`, `PermissionRequest`, same as `ENFORCING_EVENTS` in TypeScript), the install commands, and the launcher generator: `launcher::hook(target) -> LauncherHook` with the `command`, `commandWindows` and `timeout` strings. Pure functions, no I/O.
- **`toolu-runtime`** (`crates/core/runtime`): `manifest::read(plugin_root)` reads `.claude-plugin/plugin.json`, else `.codex-plugin/plugin.json`, for `version` and `hookProtocol`. `skew::assess(binary, plugin)` applies #411 and returns `Skew::{Same, Advise(..), Mismatch(..)}`.
- **`toolu-cli`** (`crates/cli`, bin `toolu`): the only binary. It implements `toolu --version`, `toolu --hook-protocol`, and `toolu [<plugin>] hook <name> [--event <Event>] [--plugin-root <dir>]`. The hook path runs the skew prelude and then the named native hook. Its `output` module owns stdout and stderr.
- **`xtask`**: `cargo xtask print-hook <plugin> <Event> <name> [--timeout N]` prints the entry JSON. `cargo xtask check-hooks` checks every native entry in `plugins/*/hooks/hooks.json` against the generator, requires its `timeout`, and checks that every `plugin.json` declares `hookProtocol` equal to `HOOK_PROTOCOL`. It is a `cargo xtask gate` step. `cargo xtask launcher-e2e --bin <path>` runs the real generated launcher against a binary installed at a fixed install directory, under `PATH=/usr/bin:/bin`. CI uses it.

The argv follows #409's seam: toolu's hooks are `toolu hook <name>`, and another plugin's are `toolu <plugin> hook <name>`. Jev decided the remaining forks in brainstorm (`docs/toolu/brainstorms/2026-10-05-native-hook-launcher.md`):

- event and plugin root travel as argv, because Claude substitutes `${CLAUDE_PLUGIN_ROOT}` as text and does not always export it;
- the semver advisory prints at SessionStart only;
- end to end at the real install directories runs in CI.

**Decisive trade-off.** The launcher spends one extra short spawn (`--hook-protocol`) per hook call. That is the price of skipping the npm wrapper `toolu` (`tools/toolu-cli/npm/package.json`). #410's budget measures the whole launcher, `sh` included.

**Reuse.** The generator keeps today's Bun chain (`TOOLU_BUN`, `command -v bun`, `~/.bun/bin/bun`) for the transition fallback and today's message rules: one line, no character that sh single quotes, cmd `echo` or JSON reinterpret, with `|` escaped as `^|` for cmd. `tooling/src/check-hooks-json.ts` skips native entries, which it recognises by `--hook-protocol` in `command`, and still gates every Bun entry.

## Interfaces / Schema

**Generated POSIX `command`** for `(plugin, Event, name)`. `<argv>` is `hook <name>` for `toolu`, `<plugin> hook <name>` otherwise:

```sh
t=; if [ -n "$TOOLU_BIN" ]; then set -- "$TOOLU_BIN"; else set -- /opt/homebrew/bin/toolu /usr/local/bin/toolu /home/linuxbrew/.linuxbrew/bin/toolu "$HOME/.local/bin/toolu" "$(command -v toolu 2>/dev/null)"; fi
for c in "$@"; do if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then case $("$c" --hook-protocol 2>/dev/null </dev/null) in ''|*[!0-9]*) ;; *) t=$c; break;; esac; fi; done
# enforcing event:
if [ -n "$t" ]; then "$t" <argv> --event <Event> --plugin-root "${CLAUDE_PLUGIN_ROOT}"; s=$?; case $s in 0|2) exit $s;; esac; printf '%s\n' "blocked: <plugin> plugin: toolu <argv> ended with status $s; the action is blocked. See docs/install.md." >&2; exit 2; fi
# context event:
if [ -n "$t" ]; then exec "$t" <argv> --event <Event> --plugin-root "${CLAUDE_PLUGIN_ROOT}"; fi
# transition fallback (removed by #440), skipped when TOOLU_BIN is set:
if [ -z "$TOOLU_BIN" ]; then b=; for c in "$TOOLU_BUN" "$(command -v bun 2>/dev/null)" "$HOME/.bun/bin/bun"; do …; done; if [ -n "$b" ] && [ -f "${CLAUDE_PLUGIN_ROOT}/hooks/dist/<name>.js" ]; then printf '%s\n' '<fallback advisory>' >&2; exec "$b" "${CLAUDE_PLUGIN_ROOT}/hooks/dist/<name>.js"; fi; fi
# missing: enforcing
printf '%s\n' 'blocked: <missing message>' >&2; exit 2
# missing: context
printf '%s\n' '{"systemMessage":"<missing message>"}'; exit 0
```

The generator joins these lines with `; `. The string contains no version, so it is byte-identical across releases.

- **Missing message:** `<plugin> plugin: toolu is not installed (checked TOOLU_BIN, /opt/homebrew/bin, /usr/local/bin, /home/linuxbrew/.linuxbrew/bin, ~/.local/bin and PATH; a TOOLU_BIN that is not a native toolu is never replaced). Install it with: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash, or: brew install falconiere/tap/toolu. Then restart the session. See docs/install.md.`
- **Fallback advisory** (stderr): `<plugin> plugin: native toolu not found, running the Bun bundle for this transition release (#425). Install toolu: <both commands>.`
- **`commandWindows`:** today's Bun chain for `hooks\dist\<name>.js`, with the missing message above (cmd-escaped). No native binary is resolved on Windows (#411).
- **Entry JSON** (`print-hook`): `{"type":"command","command":…,"commandWindows":…,"timeout":60}`, with `--timeout N` for 1 ≤ N ≤ 600. Seconds on both Claude Code and Codex.
- **Binary argv:**
  - `toolu --version` prints `toolu <semver>`.
  - `toolu --hook-protocol` prints `1`.
  - `toolu [<plugin>] hook <name> [--event <Event>] [--plugin-root <dir>]` reads the payload on stdin.
  - Unknown argv exits 64 with usage on stderr.
- **`plugin.json`:** every `plugins/*/.claude-plugin/plugin.json` and `.codex-plugin/plugin.json` gains `"hookProtocol": 1`.
- **Skew** (`plugin` = the manifest, `binary` = the compiled version and protocol):

| Case | Enforcing event | Context event |
|---|---|---|
| Same protocol, same version | runs the hook | runs the hook |
| Same protocol, binary older | runs | runs; at SessionStart one advisory line: `toolu <b> is older than the <plugin> plugin <p>; upgrade it: <cmd>` |
| Same protocol, binary newer | runs | runs; at SessionStart: `toolu <b> is newer than the <plugin> plugin <p>; update the plugins from your host's marketplace` |
| Different protocol, or a manifest that is missing, unreadable or lacks `hookProtocol` | exit 2, stderr `blocked: <plugin> plugin: hook protocol <p> does not match toolu <b> (protocol <n>); <upgrade cmd or plugin update>` | exit 0, `{"systemMessage": same text}`, hook not run |

  `<cmd>` is `brew upgrade toolu` when the binary's resolved path is under a Homebrew prefix or a `Cellar` directory. Otherwise it is the installer command.

- **Native hooks:**
  - toolu `session-start`: on payload `source` `startup` or `resume`, prints `{"systemMessage":"toolu runtime: native <version> at <current_exe>"}`, with any SessionStart advisory on the line before. Otherwise it prints the advisory alone, or nothing.
  - Any other name: an enforcing event exits 2 with `blocked: <plugin> plugin: toolu <version> has no hook <name>; upgrade it: <cmd>`. A context event prints that as a `systemMessage` and exits 0.
  - Without `--plugin-root` (direct runs such as the #409 harness), the skew prelude is skipped.
  - Without `--event`, the hook is treated as enforcing.

## Failure modes and edge cases

- **`TOOLU_BIN` set but missing, a directory, not executable, or failing the signature:** fails closed. Enforcing exits 2 with the missing message, and context prints it as a `systemMessage`. No install-directory search, no Bun fallback.
- **Npm wrapper or another non-native `toolu` first on `PATH`:** `--hook-protocol` prints a non-integer or nothing, so it is skipped and the search continues.
- **`--hook-protocol` hangs:** the host `timeout` kills the launcher. Claude and Codex treat a timed-out command as a non-blocking error, so #413's internal deadline is the real guard. Recorded as a risk, not solved here.
- **The binary aborts, segfaults, is killed or OOMs** (exit 134, 139, 137, …): on enforcing events exit 2 with `blocked:` and the status. Context events `exec`, so the host sees the raw status (non-blocking).
- **Paths with spaces** in `CLAUDE_PLUGIN_ROOT`, `HOME` or `TOOLU_BIN`: always quoted.
- **Stdin:** the probe reads `/dev/null`, so the payload reaches the hook untouched.
- **Malformed `plugin.json` or non-integer `hookProtocol`:** treated as a mismatch, so enforcing hooks fail closed. A non-semver `version` with a matching protocol advises "cannot compare" and enforces.
- **A real `toolu` already in a fixed install directory on the test machine:** the launcher tests detect it and fail with a message naming the path. They never skip.
- **Bun fallback with no bundle for `<name>`:** the missing branch runs.

## Acceptance criteria

- **AC-1:** `cargo xtask print-hook toolu PreToolUse pre-tools` prints entry JSON whose `command` matches the interface above, with a `timeout`. `cargo xtask check-hooks` passes on the repository. It fails, naming the file and the expected string, on a fixture whose native entry was hand-edited, lacks `timeout`, or whose `plugin.json` lacks or misstates `hookProtocol`.
- **AC-2:** The generated command is byte-identical when generated from two checkouts whose workspace and `plugin.json` versions differ (7.10.0 and 7.11.0).
- **AC-3:** With no `toolu` and no Bun, the generated `PreToolUse` command exits 2 with `blocked:`, "toolu is not installed" and both install commands on stderr. The `SessionStart` command exits 0 with one `systemMessage` that passes Codex's vendored session-start schema.
- **AC-4:** The built binary placed in `$HOME/.local/bin` (temporary `HOME`, `PATH=/usr/bin:/bin`) runs the generated `SessionStart` command and prints `toolu runtime: native <version> at <that path>`. In CI the same check passes with the binary installed in the Homebrew bin directory and in `/usr/local/bin` on Linux and macOS, through `cargo xtask launcher-e2e`.
- **AC-5:** Skew cases through the launcher, with the real binary and a temporary plugin root. "Runs" on PreToolUse is the toolu `session-start` hook generated under `PreToolUse`, exiting 0:
  - same version: the hook runs with no advisory;
  - plugin newer (binary older): SessionStart prints the upgrade line and PreToolUse runs (exit 0);
  - plugin older (binary newer): the update line is printed;
  - different `hookProtocol`: PreToolUse exits 2 with the upgrade command, and SessionStart prints a `systemMessage`;
  - semver-major skew with the same protocol: SessionStart advises only.
- **AC-6:** `TOOLU_BIN` pointing at a non-executable file makes the generated `PreToolUse` command exit 2 with `blocked:`, even when a valid binary sits in `$HOME/.local/bin`.
- **AC-7:** A Node script named `toolu` first on `PATH`, with the real binary in `$HOME/.local/bin`, is skipped: the hook output comes from the native binary.
- **AC-8:** A `toolu` that passes the signature check but aborts (SIGABRT), and one that dies with SIGSEGV, both make the generated `PreToolUse` command exit 2 with `blocked:` and the status.
- **AC-9:** With no `toolu` but Bun and a `hooks/dist/<name>.js` present, the generated command runs the bundle and prints the fallback advisory on stderr. The bundle's stdout and exit code pass through.
- **AC-10:** `bun run check:hooks-json` passes on a fixture plugin whose `hooks.json` holds a native entry, and still fails on a hand-edited Bun entry.
- **AC-11:** `toolu --hook-protocol` prints `1` and `toolu --version` prints `toolu 7.10.0`. Unknown argv exits 64. `cargo xtask gate` passes with `crates/cli` registered, covered and in the behaviour inventory.

## Acceptance evidence

| AC | Real input → expected observation | Boundary | Runnable check |
|---|---|---|---|
| 1 | Repository `plugins/*`; a temp copy with an edited native entry, a missing `timeout`, a missing `hookProtocol` → exit 0 / exit 1 naming each | zero native entries in the real tree | `cargo test -p xtask check_hooks`; `cargo xtask check-hooks` |
| 2 | Two temp checkouts with different versions → equal strings | version bump only | `cargo test -p xtask print_hook` |
| 3 | `sh -c <command>` with `PATH=/usr/bin:/bin`, temp `HOME`, no Bun → exit 2 and stderr / exit 0 and JSON | enforcing vs context | `cargo test -p toolu-cli --test launcher_missing` |
| 4 | Release binary in each install directory, `PATH=/usr/bin:/bin` → diagnostic names the path | install dir absent from PATH | `cargo test -p toolu-cli --test launcher_native`; CI `cargo xtask launcher-e2e --bin <dir>/toolu` |
| 5 | Temp plugin roots with edited `version` / `hookProtocol` | each of the five cases | `cargo test -p toolu-cli --test launcher_skew` |
| 6 | `TOOLU_BIN` = a 0644 file | valid binary also present | `cargo test -p toolu-cli --test launcher_resolution` |
| 7 | `#!/usr/bin/env node` script named `toolu` on PATH | wrapper first on PATH | `cargo test -p toolu-cli --test launcher_resolution` |
| 8 | Shell stand-ins that answer `--hook-protocol` then `kill -ABRT $$` / `kill -SEGV $$` | signal exits 134/139 | `cargo test -p toolu-cli --test launcher_crash` |
| 9 | Temp plugin root with a `hooks/dist/<name>.js` run by the real Bun | no toolu, Bun present | `cargo test -p toolu-cli --test launcher_fallback` |
| 10 | Temp repo fixture with one native and one edited Bun entry | native entry ignored, Bun entry still checked | `bun test tooling/src/__tests__/check-hooks-json.test.ts` |
| 11 | Built binary | unknown flag | `cargo test -p toolu-cli`; `cargo xtask gate` |

## Documentation impact

- **`docs/install.md`:** the launcher resolution order, the signature check, the missing and crash messages, the transition fallback, `TOOLU_BIN`, and the skew table, now implemented.
- **`docs/runtime.md`:** a "Native launcher" section pointing at install.md, plus `cargo xtask print-hook`.
- **AGENTS.md:** the key-files rows for `crates/cli`, the launcher generator, and `print-hook` / `check-hooks` / `launcher-e2e`. The contributing line for native hook entries, and the CI table's rust job step.

## Open Questions

None blocking. Decided here, with reasons in brainstorm:

- Argv carries the event and plugin root.
- The advisory prints at SessionStart only.
- The timeout defaults to 60 s, with `--timeout` per entry.
- `.claude-plugin/plugin.json` is read before `.codex-plugin/plugin.json`.

Before the PR, an isolated Codex profile check must confirm that Codex accepts `hookProtocol` in `.codex-plugin/plugin.json`. Claude Code's `claude plugin validate` already passes with it.
