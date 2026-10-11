# Runtime contract

The core toolu plugin launches its hooks through the installed native `toolu` binary. Other plugins still use Bun bundles until their Rust ports land. This page records both runtime paths; installation and rollback are in [install.md](install.md).

> **Core plugin.** Install the `toolu` binary once with the get.toolu.sh installer or Homebrew before enabling the toolu plugin. Its eight hook entries use native launchers. A temporary Bun bundle fallback remains for one transition release, and `TOOLU_RUNTIME=bun` selects that rollback path. See [install.md](install.md) for targets, version skew and resolution.

> **Migration status.** Leaf plugins still using TypeScript hooks or `.js` registry modules need Bun. Their own launchers continue to use the Bun path described below. OpenCode continues to enforce its adapter's TypeScript gates until #437.

## Bun prerequisite for remaining TypeScript hooks

Install Bun 1.4.x and put it on `PATH`:

```bash
curl -fsSL https://bun.sh/install | bash   # or: brew install oven-sh/bun/bun
bun --version                              # 1.4.x
```

Supported range: `>=1.4.0 <1.5.0` (docs baseline `1.4.2`). This applies when a leaf plugin or registry module still runs Bun.

## Resolution order

A hook launcher finds Bun in this order and uses the first that exists:

1. `TOOLU_BUN` — an explicit path to the `bun` executable.
2. `bun` on `PATH`.
3. `~/.bun/bin/bun`.

## When the runtime is missing

| Event kind | Behaviour |
|------------|-----------|
| Enforcing (pre-action deny, quality gate) | Fail closed: exit 2 with a message naming the prerequisite. |
| Context-only (session start, prompt hints) | Advisory: a one-line notice, no block. |

## Launcher

`hooks.json` never calls `bun` directly. Remaining TypeScript hooks use the command that `@toolu/core/launcher` generates (`packages/toolu-core/src/launcher/launcher.ts`), and `cargo xtask check-hooks` rejects drift. Print the entry to paste instead of writing it by hand:

```bash
bun run tooling/src/check-hooks-json.ts --print <plugin> <Event> <entry>   # entry = hooks/dist/<entry>.js
```

The printed hook has two strings:

- **`command`** is a POSIX `sh -c` one-liner. It takes the first executable of `$TOOLU_BUN`, `command -v bun`, `$HOME/.bun/bin/bun` and `exec`s `"${CLAUDE_PLUGIN_ROOT}/hooks/dist/<entry>.js"`, so stdin, stdout and the bundle's exit code (a deny's `2`) pass straight through. `${CLAUDE_PLUGIN_ROOT}` is its only braced variable: Claude Code substitutes it as text and Codex exports it. The line is plain POSIX so Cursor's third-party hook loader can run it too, though no Cursor host is exercised yet.
- **`commandWindows`** is Codex's `cmd.exe /C` override. It resolves `%TOOLU_BUN%`, `where bun`, `%USERPROFILE%\.bun\bin\bun.exe` in the same order and fails the same way. It is generated and gated but not exercised on Windows.

Without Bun the launcher itself answers, with no bundle involved:

| Event | stdout | stderr | exit |
|-------|--------|--------|------|
| `PreToolUse`, `PermissionRequest` (enforcing) | — | `blocked: <plugin> plugin: Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun. Install Bun 1.4.x …` | `2`: the host blocks the action |
| every other event (context-only) | `{"systemMessage":"<plugin> plugin: Bun runtime not found, …"}` | — | `0`: nothing blocks |

`systemMessage` is shown to the user and costs no model context. The payloads are checked against Codex's own output schemas (`fixtures/codex-hook-schemas/`).

**Diagnostic.** On session start and resume, toolu's native hook prints `Toolu is on!\ntoolu runtime: native <version> at <path>`. The temporary Bun fallback prints the Bun path and version. With `hooks.session-start` set to `false`, startup and resume print that title and runtime line and nothing else; clear and compact print nothing.

## Native launcher

A native hook runs the installed `toolu` binary through its generated launcher. The launcher looks in `TOOLU_BIN`, the install directories and `PATH`, checks `toolu --hook-protocol`, and falls back to the shipped Bun bundle when the binary is absent during this transition. For the core plugin, `TOOLU_RUNTIME=bun` also selects the rollback bundle when the binary is present. Print an entry with `cargo xtask print-hook <plugin> <Event> <name>` (`crates/core/protocol/src/launcher.rs`); `cargo xtask check-hooks` gates native entries and every manifest's `hookProtocol`. Resolution, messages and version skew: [install.md](install.md#hook-launcher).

Agent command tools use a non-login shell. Their plain `toolu` reachability and the SessionStart absolute-path advice are checked separately; see [Agent command shell](install.md#agent-command-shell).

## Bundles

A TypeScript hook ships as a committed single-file ESM bundle. It is never installed or compiled on the user's machine. Each top-level `plugins/<name>/hooks/src/<entry>.ts` is built by `bun run build:plugins` into `plugins/<name>/hooks/dist/<entry>.js`, which inlines `@toolu/core` and its dependencies and runs with `bun` and no `node_modules`. The build pins its working directory to the repository root, so the output is byte-identical on every machine running the same Bun. `bun run check:plugin-bundles` rebuilds into a temp directory and fails CI when a committed bundle drifts from its source, is missing, or has no source. Published packages carry `hooks/dist`, never `hooks/src`.

A legacy skill CLI that starts with `#!/usr/bin/env bun` builds to an executable `hooks/dist/<entry>.js`, and the drift check fails if a committed one loses its exec bit. The native toolu-review writer is `toolu review write-state`; SessionStart keeps its former stable helper as a shell shim until #440.

**Startup hooks.** toolu-review's SessionStart entries use the native launcher. The Codex dependency checks of pr-babysit and the three quality plugins are native (`toolu <plugin> hook check-toolu`, #433, #426–#428); epic-orchestrator's three SessionStart entries are native (`toolu epic-orchestrator hook …`, #435). A publisher never replaces a regular file at its stable path, only a symlink. A legacy published Bun CLI still needs `bun` on the agent command shell's `PATH`; the toolu-review shim does not.

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
through `@toolu/core/resources/jobs`. Workers use `toolu epic job -- <command>`
for other expensive commands. These limits cover managed commands; native
host tools outside that path are checked through workload inventory at teardown.
Uncertain launches and incomplete cleanup retain ownership. See the
[epic orchestrator contract](../plugins/epic-orchestrator/README.md) for policy,
host verification limits and recovery.

## Why not node

There is no `node` fallback. Measured warm hook startup on macOS: bash plus jq 6.3 ms, bun 5.2 ms (12.3 ms with zod), bun single-file bundle 6.4–8.6 ms, **node 49.7 ms**, python3 23.5 ms. Node is roughly 10x slower per hook than Bun (49.7 ms versus 5.2 ms), so a fallback would silently slow every tool call.

## Decision record

- **Historical decision (#247):** run plugins, hooks, gates, scripts and tests on TypeScript under Bun while retiring bash and bats. Epic #402 supersedes this for ported hooks, starting with toolu's native entries in #425.
- **Why:** every host except OpenCode spawns command hooks in any language, and OpenCode loads TypeScript in-process, so TypeScript on Bun is the one language every host can run. The measurements show parity, not a speed-up: Bun (5.2 ms) is within 1.1 ms of bash plus jq (6.3 ms), so the rewrite does not slow hooks. The reason to migrate is one language across all hosts, not speed.
- **Evidence:** the latency measurements above (macOS, recorded in [epic #247](https://github.com/Falconiere/toolu/issues/247)) support only that Bun does not regress hook startup and that node is 8x slower than Bun; they do not by themselves justify the rewrite.
- **Current consequence:** Bun 1.4.x remains necessary for unported leaf hooks, registry modules and the temporary rollback path. The core toolu hooks default to the native binary.
- **Supersedes:** the #203 non-goal of keeping Claude Code and Codex free of a mandatory Bun dependency.
