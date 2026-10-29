# Runtime contract

Every toolu hook, skill command and agent command runs the installed native `toolu` binary. There is no other runtime: Bun is not used or needed on Claude Code, Codex or Muse (#440). Installation, targets, version skew and binary resolution are in [install.md](install.md).

> **Install once.** Install `toolu` with the get.toolu.sh installer or Homebrew before you enable a toolu plugin:
>
> ```bash
> curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash   # or: brew install falconiere/tap/toolu
> toolu --version
> ```

## Hook launcher

`hooks.json` never names a binary path or a runtime. Every entry is the command `toolu_protocol::launcher` generates (`crates/core/protocol/src/launcher.rs`). Print the entry to paste instead of writing it by hand; `cargo xtask check-hooks` rejects any entry that differs from the generator or is not a native launcher:

```bash
cargo xtask print-hook <plugin> <Event> <name>   # runs `toolu [<plugin>] hook <name>`
```

The printed hook has two strings:

- **`command`** is a POSIX `sh -c` one-liner. It tries `$TOOLU_BIN` alone when that is set, else `/opt/homebrew/bin`, `/usr/local/bin`, `/home/linuxbrew/.linuxbrew/bin`, `~/.local/bin` and then `PATH`. It keeps the first executable whose `toolu --hook-protocol` prints an integer. For an enforcing event it runs the hook without `exec`, passes 0 and 2 through, and turns any other status (a crash or a signal) into exit 2 with `blocked:`, naming the hook and its event. A context event `exec`s the binary, so a host that kills the hook on timeout kills toolu itself; the binary names the hook in its own panic or error report. `${CLAUDE_PLUGIN_ROOT}` is its only braced variable: Claude Code substitutes it as text and Codex exports it.
- **`commandWindows`** is Codex's `cmd.exe /C` override. No Windows binary is released yet (#411), so it reports the missing binary exactly as `command` does without one.

Without a usable binary the launcher itself answers:

| Event | stdout | stderr | exit |
|-------|--------|--------|------|
| `PreToolUse`, `PermissionRequest` (enforcing) | — | `blocked: <plugin> plugin: no usable native toolu - checked TOOLU_BIN, … Install toolu with: …` | `2`: the host blocks the action |
| every other event (context-only) | `{"systemMessage":"<plugin> plugin: no usable native toolu - …"}` | — | `0`: nothing blocks |

A `TOOLU_BIN` that does not name a native `toolu` is never replaced by another candidate: it fails closed the same way. `systemMessage` is shown to the user and costs no model context. The payloads are checked against Codex's own output schemas (`fixtures/codex-hook-schemas/`).

**Diagnostic.** On session start and resume, toolu's hook prints `Toolu is on!\ntoolu runtime: native <version> at <path>`. With `hooks.session-start` set to `false`, startup and resume print that title and runtime line and nothing else; clear and compact print nothing.

On Muse, startup and resume put that title, runtime line and any session guidance in one `systemMessage`. Prompt hints use the same field. The native binary check shares a once-per-session marker across plugin hooks and gives a Muse install command or an absolute native path when plain `toolu` is unavailable to agent commands.

On OpenCode, hooks.json is not run by the host: each installed plugin's generated `server.js` resolves `toolu` in the same order and hands every hook call to `toolu opencode bridge`, which runs the plugin's `hooks.json` entries as the binary itself. A missing binary refuses tool calls and is logged on the other hooks ([OpenCode](opencode.md)).

Agent command tools use a non-login shell. Their plain `toolu` reachability and the SessionStart absolute-path advice are checked separately; see [Agent command shell](install.md#agent-command-shell).

## Migrating from the Bun runtime

Releases up to #440 shipped Bun bundles beside the native entries. What changed:

- **No fallback.** A missing binary no longer runs a Bun bundle; enforcing hooks fail closed and context hooks print the install advice.
- **`TOOLU_RUNTIME=bun` is ignored.** The rollback switch of #425 is gone; the native hook runs.
- **No stable-path shims.** `<config>/jev/jev.sh`, `<config>/statusline/statusline.sh` and `<config>/toolu-review/write-state.sh` are no longer published. Each plugin's SessionStart removes the link an older version published there and leaves a file of your own alone. Run `toolu jev …`, `toolu statusline render` and `toolu review write-state` instead. The statusline SessionStart rewrites a `settings.json` `statusLine` that ran the old path, and says so when it cannot.
- **`.js` registry modules no longer run.** A third-party module must be an executable `.sh` module ([registry.md](registry.md#executable-modules)). SessionStart names each `.js` module it finds.
- **`@toolu/core` is deprecated.** The TypeScript library is no longer published or maintained.

## Managed epic workloads

Epic orchestration runs in the native `toolu epic` engine and verbs (#435), with
bounded subprocesses, drained output, native exit status and process-group
cancellation. Shared resource state defaults to `~/.local/state/toolu/resources`
(`TOOLU_RESOURCE_HOME` overrides it). The plugin ships no scripts or bundles of
its own.
Agent slots and expensive-job slots are separate; conservative defaults allow
three agents and one managed job. Load, available memory and Linux CPU steal
inform sustained admission holds with recovery hysteresis. On macOS, available
memory comes from `vm_stat` (free + inactive + speculative pages), and
`"pressure": false` in the resource root's `policy.json` disables the hold.

A binding in the worktree's Git directory automatically admits plan-ledger checks
through `toolu_engine::resources`. Workers use `toolu epic job -- <command>`
for other expensive commands. These limits cover managed commands; native
host tools outside that path are checked through workload inventory at teardown.
Uncertain launches and incomplete cleanup retain ownership. See the
[epic orchestrator contract](../plugins/epic-orchestrator/README.md) for policy,
host verification limits and recovery.

## Bounded subprocess pipe cleanup

`toolu_runtime::process::run` starts a subprocess in its own process group and
drains stdout and stderr while feeding stdin. A descendant that calls `setsid`
can escape that group and hold an inherited pipe open. The runtime uses
nonblocking pipe I/O so that, after settlement succeeds (including a handled
timeout), it gives an unfinished feeder up to 250 ms to complete, then cancels
the workers and waits for them against one shared 250 ms cleanup deadline.
Settlement errors skip the feeder grace and cancel immediately. These waits
are additional to the command timeout, group termination, and existing final
stream-drain grace.
This releases their descriptors, input copy, and observer captures even while
the escaped descendant is still alive. The existing group termination and
reaping path remains in force. An observer callback that itself never returns
cannot be interrupted; the caller still stops waiting at its cleanup deadline,
but that callback's reader retains its descriptor and captures until it returns.

The following release-build probe ran on Darwin 27.0.0 arm64 on 2026-10-29,
five times per pipe. A real Perl descendant called `setsid`, inherited exactly
one pipe, wrote its PID to a temporary file, and slept without reading or
closing that pipe. The parent `sh` exited; stdin runs fed 8 MiB and each run
had a 150 ms timeout. Each escaped descendant was terminated only after the
post-return measurement. Counts were measured in the caller process. The
baseline was commit `611f0d0d2f9e60c9997ebd3899a8e60433f62cbb`; the after
build used this fix. The same workload and assertions are in
`crates/core/runtime/src/tests/escaped_test.rs` and can be rerun with
`cargo test -p toolu-runtime --release escaped_resources -- --test-threads=1 --nocapture`.

| Held pipe | Before: threads / fds / observer refs after return | After: threads / fds / observer refs after return | Return time before → after |
| --- | --- | --- | --- |
| stdin | 2 / 5 / 1 | 1 / 4 / 1 | 284–292 ms → 299–321 ms |
| stdout | 2 / 5 / 2 | 1 / 4 / 1 | 402–415 ms → 420–430 ms |
| stderr | 2 / 5 / 2 | 1 / 4 / 1 | 401–419 ms → 417–428 ms |

Idle baseline was 1 thread, 4 descriptors, and 1 caller-owned observer reference.
The before build returned to baseline only after the escaped descendant was
terminated. These timings describe this workload and machine; they are not a
general subprocess speedup claim.

## Commands that take over the terminal

`toolu sandbox run` and `toolu sandbox shell` end in `toolu_runtime::process::exec`,
which replaces the toolu process with the command (`execvp`). The command keeps
toolu's pid, process group, terminal and standard streams, so its output, its
signals and its exit status are the caller's own, with nothing buffered in
between. `exec` finds the program on `PATH` first and refuses a missing one
before std changes any process state.

## Decision record

- **Decision (epic #402):** every plugin is a Rust crate behind one `toolu` binary plus Markdown; hooks run the binary through the generated launcher. #425 switched the core plugin and kept a Bun fallback for one release; #440 removed the fallback, the bundles and the TypeScript tree.
- **Why:** hook memory and CPU. Every tool call spawns the pre- and post-tool hooks. Bun alone costs 13 MB RSS, and the Bun dispatchers cost 37–56 MiB and 57–166 ms CPU per call at p50 on the CI runners ([Bun baseline](resource-budgets.md#bun-baseline)); the native hooks stay within the budgets of [resource-budgets.md](resource-budgets.md), measured by `cargo xtask bench-hooks`.
- **Superseded:** the TypeScript-on-Bun decision of epic #247.
