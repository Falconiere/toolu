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

## Bundles

A TypeScript hook ships as a committed single-file ESM bundle. It is never installed or compiled on the user's machine. Each top-level `plugins/<name>/hooks/src/<entry>.ts` is built by `bun run build:plugins` into `plugins/<name>/hooks/dist/<entry>.js`, which inlines `@toolu/core` and its dependencies and runs with `bun` and no `node_modules`. The build pins its working directory to the repository root, so the output is byte-identical on every machine running the same Bun. `bun run check:plugin-bundles` rebuilds into a temp directory and fails CI when a committed bundle drifts from its source, is missing, or has no source. Published packages carry `hooks/dist`, never `hooks/src`.

## Why not node

There is no `node` fallback. Measured warm hook startup on macOS: bash plus jq 6.3 ms, bun 5.2 ms (12.3 ms with zod), bun single-file bundle 6.4–8.6 ms, **node 49.7 ms**, python3 23.5 ms. Node is roughly 10x slower per hook than Bun (49.7 ms versus 5.2 ms), so a fallback would silently slow every tool call.

## Decision record

- **Decision:** run every plugin, hook, gate, script, and test on TypeScript under Bun; remove bash and bats when nothing remains.
- **Why:** every host except OpenCode spawns command hooks in any language, and OpenCode loads TypeScript in-process, so TypeScript on Bun is the one language every host can run. The measurements show parity, not a speed-up: Bun (5.2 ms) is within 1.1 ms of bash plus jq (6.3 ms), so the rewrite does not slow hooks. The reason to migrate is one language across all hosts, not speed.
- **Evidence:** the latency measurements above (macOS, recorded in [epic #247](https://github.com/Falconiere/toolu/issues/247)) support only that Bun does not regress hook startup and that node is 8x slower than Bun; they do not by themselves justify the rewrite.
- **Consequence:** Bun 1.4.x is a documented prerequisite for every host; a missing runtime fails closed on enforcing events.
- **Supersedes:** the #203 non-goal of keeping Claude Code and Codex free of a mandatory Bun dependency.
