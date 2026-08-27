# comemory

Persistent agent memory + code-index search (comemory): a skill, a scoped
wrapper, host-native setup workflow, `PreToolUse` scope enforcement, and a
`SessionStart` memory-count publisher — registered into the toolu hook engine.

## Install

```text
/plugin install comemory@toolu
```

```bash
codex plugin add comemory@toolu
```

Requires the `toolu` plugin.

## What it provides

- **`agent-memory` skill** — an always-active protocol: recall from memory *first* (before reading files), and save decisions, conventions, bugs, and discoveries proactively. A search miss is an obligation to save the finding back. The mandate is **opt-in**: it activates only after you run `/comemory:setup` on Claude or `$comemory:setup` on Codex in a repo — until then the protocol stays dormant.
- **`project-skills` skill** — procedural memory for this repo. After a proven class-level workflow the agent writes `<repo>/.toolu/skills/<name>/SKILL.md` via `skills.sh create`. SessionStart injects a compact name+description index; a `Read` of the file counts as use. A daily Stop curator archives unused *agent-created* skills (30d stale / 90d archive) — never deletes, never touches marketplace plugin skills.
- **setup workflow** — the Claude command and Codex skill share one body. It detects the `comemory` binary, guides installation if missing or old (never runs a package manager), then wires the repo's data dir, git index hooks, initial index, and completions hint.
- **scoped wrapper** (`comemory.sh`) with `delete` / `context` verbs, plus `PreToolUse` scope enforcement keeping memory access inside the current repo. `skills.sh` is published next to it.
- **`comemory-status` (`SessionStart`)** — publishes the per-repo memory count the statusline's `[COMEMORY:N]` segment reads.

## The comemory binary

The plugin wraps the standalone `comemory` binary (memory + code-index backend) and **no-ops entirely if it is absent**. It is **not** on crates.io — install it once:

```bash
brew install Falconiere/tap/comemory
# or:
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Falconiere/comemory/releases/latest/download/comemory-installer.sh | sh
```

Then run `/comemory:setup` on Claude or `$comemory:setup` on Codex.
