# comemory — Persistent Agent Memory

**Type:** Code Intel | **Version:** 4.5.0 | **Depends on:** `toolu`

Persistent cross-session agent memory + code-index search — a skill, a scoped
wrapper, shared Claude `/comemory:setup` and Codex `$comemory:setup` workflow,
`PreToolUse` scope enforcement, and a `SessionStart` memory-count publisher.

## Install

```text
/plugin install comemory@toolu
```

```bash
codex plugin add comemory@toolu
```

Requires the standalone `comemory` binary (**not** on crates.io):

```bash
brew install Falconiere/tap/comemory
# or the curl installer:
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Falconiere/comemory/releases/latest/download/comemory-installer.sh | sh
```

Then run the setup command:

```text
/comemory:setup
```

Use `$comemory:setup` on Codex.

This detects the binary (guides install if missing/old), then wires: data directory, git hooks that auto-refresh the code index on commit/merge/checkout, an initial index, and shell completions.

## What It Provides

### 1. `agent-memory` Skill (Always Active)

Mandates recalling from memory **first** before reading files. A search miss is an obligation to save the finding back.

### 2. Scoped Wrapper (`comemory.sh`)

With `delete`, `context`, `search`, `save`, `list`, `stats`, and `feedback` verbs. Auto-injects `--repo <current-repo>` so memory is always scoped.

### 3. Scope Enforcement (`PreToolUse`)

Blocks raw `comemory` calls that omit `--repo`. The wrapper auto-detects the git repo and injects the scope.

### 4. Memory-Status Publisher (`SessionStart`)

Publishes per-repo memory count that the statusline's `[COMEMORY:N]` segment reads.

## Usage Examples

### Search Memory Before Reading Files

```bash
# At the start of every task — recall first
comemory.sh search "error handling rules in middleware"

# Architecture question → search past decisions
comemory.sh search "architecture auth module" --kind decision

# Where is the code for X? → search cached file maps
comemory.sh search "file-map pipeline queries" --kind discovery

# What was decided about X? → search decisions
comemory.sh search "decision state management" --kind decision

# Widen the candidate window
comemory.sh search "auth" --k 20
```

### Save What You Learn

```bash
# Scope: toolu (announce the repo scope before every call)

# Save a design decision
comemory.sh save "JWT auth middleware" \
  "**What**: Added JWT validation middleware
   **Why**: API routes needed authentication
   **Where**: src/middleware/auth.ts
   **Learned**: Must set httpOnly flag on cookies" \
  --kind decision --tags "auth,middleware"

# Save a bug fix with root cause
comemory.sh save "Canvas drag bug: offset doubled on retina" \
  "**What**: Drag offset was being multiplied by devicePixelRatio twice
   **Where**: src/canvas/drag.ts:42
   **Fix**: Remove the second multiplication in handleDragMove" \
  --kind bug --tags "canvas,drag,retina"

# Save a code pattern
comemory.sh save "Query hook pattern" \
  "**Pattern**: useMutation wraps trpc hooks with loading/error states
   **Where**: src/hooks/useMutation.ts
   **Usage**: const { mutate, isLoading } = useMutation('endpoint')" \
  --kind pattern --tags "query,hooks"

# Save a gotcha
comemory.sh save "oRPC client types require explicit generic" \
  "**Gotcha**: oRPC client.infer<Router> needs the Router type passed explicitly
   **Where**: src/api/client.ts
   **Fix**: const client = createClient<AppRouter>(...)" \
  --kind note --tags "gotcha,orpc-client-types"

# Supersede an outdated memory (replace, don't duplicate)
comemory.sh save "Updated auth middleware" "..." --kind decision --supersedes abc12345
```

### Browse & Manage Memories

```bash
# List all memories in the current repo
comemory.sh list

# List only decisions
comemory.sh list --kind decision

# List all pattern memories
comemory.sh list --kind pattern

# Check data directory + index health
comemory.sh stats

# Delete a memory by its 8-hex id (moves to .trash/)
comemory.sh delete abc12345
```

### Context Lookup

```bash
# Bundles matching code symbols + related memories
comemory.sh context "authMiddleware" --k 5
```

### Feedback Loop

After using a recalled memory, close the loop to sharpen future recall:

```bash
# From a prior search with --json, use the query_id
comemory.sh search "auth pattern" --json    # returns query_id in envelope
comemory.sh feedback <query_id> --used abc1,def2 --irrelevant ghi3
```

The retrieval-quality loop verbs (`mine`, `tune`, `eval`, `prune`, `gc`, `rebuild`) run automatically once per day via the toolu `SessionEnd` hook — local and token-free.

## Project skills (procedural memory)

Facts stay in comemory. Recurring **procedures** live as agent-created `SKILL.md` files:

```
<repo>/.toolu/skills/<name>/SKILL.md   # committed
<repo>/.toolu/skills/.usage.json       # gitignored (local telemetry)
<repo>/.toolu/skills/.archive/         # gitignored (recoverable)
```

```bash
# After a proven class-level workflow (not a ticket narrative):
skills.sh create deploy-staging --description "Deploy this repo to staging. Use when shipping a branch to staging." --file ./skill.md

skills.sh list
skills.sh pin deploy-staging
skills.sh curate --dry-run
skills.sh restore deploy-staging
```

The published wrapper is `${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/comemory/skills.sh` (Codex: `$CODEX_HOME/comemory/skills.sh`). SessionStart injects at most 20 name+description lines; `Read` the `SKILL.md` to load it (that Read is what increments use). A Stop hook archives unused agent-created skills after 90 idle days (stale at 30). It **never auto-deletes** and **never touches** marketplace `plugins/*/skills/`. Disable with `skills.comemory: false` or `projectSkills.enabled: false`.

### Post-Compaction Recovery

When you see a compaction message or "FIRST ACTION REQUIRED":

```bash
comemory.sh search "<current task / open thread>"
# Recover relevant prior context from memory, then continue working
```

## Memory Kinds & Tags

| What You Learned | `--kind` | Example Tags |
|-----------------|----------|-------------|
| Architecture / design decision | `decision` | `architecture,api-routing` |
| File / module location mapping | `discovery` | `file-map,pipeline-queries` |
| Reusable code pattern | `pattern` | `query-hooks` |
| Decision with trade-off rationale | `decision` | `state-management` |
| Bug fix with root cause | `bug` | `canvas-drag-offset` |
| Project convention | `convention` | `import-aliases` |
| Gotcha / edge case / nuance | `note` | `gotcha,orpc-client-types` |

## The Search Stack

```
Need to understand something?
│
├─ comemory.sh search → past decisions (ALWAYS FIRST)
├─ ast-grep → structural code patterns (AST)
├─ Grep → exact literals (text)
├─ Glob → path patterns
└─ Read → open known file
```

Skip memory + structural search only when the user explicitly says to read/edit a specific file, or when running tests/builds/git commands.

## Self-Check After Every Task

> "Did I just make a decision, fix a bug, learn something non-obvious, or establish a convention? If yes → save NOW."
> "Did I just hit/fix a quality or e2e gate? If yes → save NOW."
> "Did I name the repo scope before calling comemory?"

## Configuration

Disable via `toolu.config.json`:

```json
{ "version": 1, "skills": { "comemory": false } }
```

When disabled: no recall hints, no install nag, no comemory status in the statusline.

## Version Requirement

toolu targets **comemory ≥ 0.8.0**. An older binary emits a non-fatal upgrade WARN at session start — basics (`search`/`save`/`list`) still work. Upgrade:

```bash
brew upgrade Falconiere/tap/comemory
comemory:setup   # verify and re-wire
```
