# Native hook launcher — brainstorm (#412)

**Date:** 2026-10-05   **Mode:** Delivery, Full path (public hook interface, fail-closed security boundary)

## Capsule

- **Outcome:** a generated POSIX `sh -c` launcher that finds the installed native `toolu` (`TOOLU_BIN`, the fixed install directories, then `PATH`), skips anything that fails `toolu --hook-protocol`, never `exec`s on enforcing events so a crash becomes exit 2, falls back to the shipped Bun bundle during the transition, and otherwise fails closed with both install commands. The binary applies the #411 skew rule from the calling plugin's `plugin.json`. `cargo xtask print-hook` emits entries and `cargo xtask check-hooks` gates them.
- **Material defaults / non-goals:** no production `hooks.json` entry switches to native here (#425 does, after the ports); the binary implements only `--version`, `--hook-protocol` and the `hook` prelude with one native hook, toolu's `session-start` diagnostic; clap, namespaces and `toolu commands --json` stay with #442; the internal hook deadline belongs to the hook main wrapper (#413).
- **Repository evidence:** `packages/toolu-core/src/launcher/launcher.ts` (today's Bun launcher contract and messages), `tooling/src/check-hooks-json.ts` (the TS gate that must skip native entries), `tools/toolu-conformance/src/harness/entry-command.ts` (#409 argv: `toolu hook <entry>` for toolu, `toolu <plugin> hook <entry>` otherwise), `tooling/conventions/guardrails/rust/{layers,rules}.json` (only `crates/cli` builds a binary; stdio in `toolu-cli::output`; env in `toolu-runtime`), `docs/install.md` (#411 policy).
- **Risk:** the signature probe adds one short spawn per hook call (counted by #410's budget); tests that use the fixed install directories fail on a machine that already has `toolu` installed there, so they check that precondition and fail with a clear message.
- **Handoff:** spec.

## Axes and decisions

| Axis | Decision | Evidence | Jev |
|---|---|---|---|
| Binary | Create a minimal `crates/cli` (package `toolu-cli`, bin `toolu`), hand-parsed argv; #442 replaces the parser with clap | No binary exists; #442 is blocked on #410; only `crates/cli` may build a binary | `minimal_cli` 0.75 |
| Event and plugin root | argv flags `--event <Event> --plugin-root "${CLAUDE_PLUGIN_ROOT}"`, fixed per entry | Claude substitutes `${CLAUDE_PLUGIN_ROOT}` as text and does not always export it; env reads are restricted to `toolu-runtime` | `argv_flags` 0.42 vs `env_vars` 0.41 (tie; argv keeps the contract visible in `hooks.json`) |
| Semver advisory | SessionStart only | Issue scenario names session start; one line per session avoids noise on every prompt | `session_start_only` 0.84 |
| End to end at real install directories | CI step copies the release binary into the Homebrew bin directory and `/usr/local/bin` in turn and runs `cargo xtask launcher-e2e`; cargo tests cover `~/.local/bin` under a temporary `HOME` | Install channels (#457) do not exist yet | `ci_install_dirs` 0.97 |
| Code placement | Launcher text and event classes in `toolu-protocol`; manifest read and skew rule in `toolu-runtime`; argv, stdout and stderr in `toolu-cli` | Layer table and capability owners | — |
| Windows | `commandWindows` keeps today's Bun chain for the bundle and prints the install hint when nothing is found | #411: Windows deferred, `commandWindows` prints the install hint | — |

## Rejected alternatives

- **No binary, shell stand-ins only:** cannot show a native hook running end to end or the skew rule in the real binary.
- **Prototype binary inside xtask:** a second throwaway binary that #442 would delete.
- **Reading `hook_event_name` from stdin and `CLAUDE_PLUGIN_ROOT` from the environment:** Claude does not always export the variable, and a crash before parsing would lose the event class.
- **Advisory on every context event:** one line per prompt for a condition that only changes on upgrade.
