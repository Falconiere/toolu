# Runtime contract

Every toolu plugin runs on one runtime: **Bun**. This page is the contract; other docs link here instead of restating it. It retires the [#203](https://github.com/Falconiere/toolu/issues/203) constraint that Claude Code and Codex gain no mandatory Bun dependency. Epic: [#247](https://github.com/Falconiere/toolu/issues/247).

> **Migration status.** Plugins move from bash to TypeScript one at a time (epic #247). Until a plugin's port merges, its bash hooks still ship and still need `bash` and `jq`. Bun is a prerequisite for every host now, so installs do not change again as ports land.

## Prerequisite

Install Bun 1.4.x and put it on `PATH`:

```bash
curl -fsSL https://bun.sh/install | bash   # or: brew install oven-sh/bun/bun
bun --version                              # 1.4.x
```

Supported range: `>=1.4.0 <1.5.0` (docs baseline `1.4.2`). This applies to Claude Code, Codex, Cursor, OpenCode, and Hermes alike. `epic-orchestrator` already requires `bun`.

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

`hooks.json` never calls `bun` directly. Every TypeScript hook uses the command that `@toolu/core/launcher` generates (`packages/toolu-core/src/launcher/launcher.ts`), and `bun run check:hooks-json` (`tooling/src/check-hooks-json.ts`, part of `bun run test`) fails when a launcher entry differs from it. Print the entry to paste instead of writing it by hand:

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

`systemMessage` is shown to the user and costs no model context. The payloads are checked against Codex's own output schemas (`tooling/fixtures/codex-hook-schemas/`).

**Diagnostic.** On session start and resume, toolu's `hooks/dist/session-start.js` reports which runtime the hooks use on the second line of its `systemMessage`, after the event title: `Toolu is on!\ntoolu runtime: bun <version> at <path>`. With `hooks.session-start` set to `false` it prints that line alone: `{"systemMessage":"toolu runtime: bun <version> at <path>"}`.

## Bundles

A TypeScript hook ships as a committed single-file ESM bundle. It is never installed or compiled on the user's machine. Each top-level `plugins/<name>/hooks/src/<entry>.ts` is built by `bun run build:plugins` into `plugins/<name>/hooks/dist/<entry>.js`, which inlines `@toolu/core` and its dependencies and runs with `bun` and no `node_modules`. The build pins its working directory to the repository root, so the output is byte-identical on every machine running the same Bun. `bun run check:plugin-bundles` rebuilds into a temp directory and fails CI when a committed bundle drifts from its source, is missing, or has no source. Published packages carry `hooks/dist`, never `hooks/src`.

A skill CLI is a bundle too. An entry that starts with `#!/usr/bin/env bun` builds to an executable `hooks/dist/<entry>.js`, and the drift check fails if a committed one loses its exec bit. The plugin's SessionStart hook symlinks it to a stable path such as `<config>/exa-search/search.sh`, so agents run it by path and `bun` must be on `PATH`. exa-search, context7, agent-browser, jira and toolu-review's `write-state.sh` ship this way (#270, #272, #269).

**Startup hooks.** The SessionStart hooks of context7, exa-search, agent-browser, jira, toolu-review and jev, and the Codex dependency checks of pr-babysit, python-quality, rust-quality and epic-orchestrator, are bundles on `@toolu/core/startup` (#269). A publisher never replaces a regular file at its stable path, only a symlink. When the launcher found Bun through `TOOLU_BUN` or `~/.bun/bin` but `bun` is not on `PATH`, it still publishes and prints one stderr line (`<plugin>: bun not found on PATH — the <tool> needs Bun 1.4.x …`), because the published CLI's shebang needs `bun` on `PATH`. With no Bun at all, the launcher's `systemMessage` replaces the hook and nothing is published.

## Why not node

There is no `node` fallback. Measured warm hook startup on macOS: bash plus jq 6.3 ms, bun 5.2 ms (12.3 ms with zod), bun single-file bundle 6.4–8.6 ms, **node 49.7 ms**, python3 23.5 ms. Node is roughly 10x slower per hook than Bun (49.7 ms versus 5.2 ms), so a fallback would silently slow every tool call.

## Decision record

- **Decision:** run every plugin, hook, gate, script, and test on TypeScript under Bun; remove bash and bats when nothing remains.
- **Why:** every host except OpenCode spawns command hooks in any language, and OpenCode loads TypeScript in-process, so TypeScript on Bun is the one language every host can run. The measurements show parity, not a speed-up: Bun (5.2 ms) is within 1.1 ms of bash plus jq (6.3 ms), so the rewrite does not slow hooks. The reason to migrate is one language across all hosts, not speed.
- **Evidence:** the latency measurements above (macOS, recorded in [epic #247](https://github.com/Falconiere/toolu/issues/247)) support only that Bun does not regress hook startup and that node is 8x slower than Bun; they do not by themselves justify the rewrite.
- **Consequence:** Bun 1.4.x is a documented prerequisite for every host; a missing runtime fails closed on enforcing events.
- **Supersedes:** the #203 non-goal of keeping Claude Code and Codex free of a mandatory Bun dependency.
