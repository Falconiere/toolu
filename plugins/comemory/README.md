# comemory

Persistent agent memory + code-index search (comemory): a skill, a scoped wrapper, a `/comemory:setup` command, `PreToolUse` scope enforcement, and a `SessionStart` memory-count publisher — registered into the toolu hook engine.

## Install

```
/plugin install comemory@toolu
```

Requires the `toolu` plugin.

## What it provides

- **`agent-memory` skill** — an always-active protocol: recall from memory *first* (before reading files), and save decisions, conventions, bugs, and discoveries proactively. A search miss is an obligation to save the finding back.
- **`/comemory:setup` command** — detects the `comemory` binary, guides the install if it is missing or too old (never runs a package manager itself), then wires the repo: data dir, git hooks that auto-refresh the code index on commit/merge/checkout, an initial index, and a completions hint.
- **scoped wrapper** (`comemory.sh`) with `delete` / `context` verbs, plus `PreToolUse` scope enforcement keeping memory access inside the current repo.
- **`comemory-status` (`SessionStart`)** — publishes the per-repo memory count the statusline's `[COMEMORY:N]` segment reads.

## The comemory binary

The plugin wraps the standalone `comemory` binary (memory + code-index backend) and **no-ops entirely if it is absent**. It is **not** on crates.io — install it once:

```bash
brew install Falconiere/tap/comemory
# or:
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Falconiere/comemory/releases/latest/download/comemory-installer.sh | sh
```

Then run `/comemory:setup`.
