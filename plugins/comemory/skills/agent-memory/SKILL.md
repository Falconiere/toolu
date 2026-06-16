---
name: agent-memory
description: "ALWAYS ACTIVE — Persistent memory protocol. You MUST save decisions, conventions, bugs, and discoveries to comemory proactively. Do NOT wait for the user to ask."
---
# Agent Memory-First Protocol
You have comemory persistent memory accessed through the **scoped wrapper**.
This protocol is **MANDATORY and ALWAYS ACTIVE**.

## Wrapper path — `comemory.sh`

Invoke the wrapper at the **stable published path** (a symlink the plugin's
SessionStart hook refreshes every session):

```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" <subcmd> …
```

`$CLAUDE_CONFIG_DIR` IS exported into the Bash tool's subshell; `$CLAUDE_PLUGIN_ROOT` is **NOT** — it is only set for hook subprocesses. Using the plugin-root path from a Bash tool call expands to an empty string and runs `/skills/.../comemory.sh: No such file or directory`, which prints zero bytes to stdout and looks like "no memory hits". **Always use the published path above.**

Repo-checkout fallback (when running tests/scripts directly against this repo, with the plugin not installed): `plugins/comemory/skills/agent-memory/scripts/comemory.sh`.

## Hard Constraints (Always)
- Memory is required for recall and save.
- **Recall FIRST, from the repository.** Before reading files to understand anything, run `comemory.sh search` — at the start of every task and whenever you hit an unfamiliar area. This is not optional and not something to ask permission for.
- **A search miss is an obligation, not a dead end.** When recall returns nothing, do the native search (ast-grep → Grep → Read), and the moment you learn something reusable, **save the finding back** so the next miss becomes a hit. The wrapper prints this reminder on every empty `search`; honor it. Save patterns, bug fixes, conventions, decisions, gotchas, and nuances.
- **EVERY save/recall MUST be scoped to a repo — always pass `--repo`.** The wrapper auto-detects the current repo via `detect_project_name` (git toplevel basename) and injects `--repo` for you; never strip it, and never fall back to a raw `comemory` call that omits it. Raw `comemory` calls without `--repo` are blocked by the `comemory-scope` pre-tool hook.
- Before ANY save or recall, the agent MUST state which repo it is scoping (e.g. `Scope: toolu`). Wrong scope = wrong memories = wasted tokens or contaminated context.
- Global quality gate is blocking — do not switch tasks while any errors/warnings/tests fail (even unrelated).
- Test policy — NO mock-data tests. Use real-world data/integration paths.
- Keep memory entries compact, structured, and searchable.

## Repo Scope — How to Decide

| Situation | Scope |
|---|---|
| Working in CWD that is a git repo | basename of `git rev-parse --show-toplevel` (the wrapper handles this) |
| Override needed (cross-repo work) | `MY_CLAUDE_COMEMORY_REPO=<name>` env var before the call |
| Not in a git repo | Wrapper falls back to `unknown` — set `MY_CLAUDE_COMEMORY_REPO` explicitly |

Announce the scope in user-facing text before performing the operation. Example:
> Scoping comemory to **toolu** for recall on "error handling rules".

## CLI Reference — Use the Wrapper

The wrapper at `"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" <subcmd>` auto-injects `--repo <current-repo>`. **Never use MCP tools for comemory.** Raw `comemory` invocations are denied by the `comemory-scope` hook unless they include `--repo`.

### Save a memory
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" save "<title>" "<body>" --kind KIND --tags "a,b"
```
- `<title>`: Short, searchable title (required)
- `<body>`: Structured content (required) — the wrapper folds title + body into one memory
- `--kind`: `decision` | `bug` | `convention` | `discovery` | `pattern` | `note` (default `note`)
- `--tags`: comma-separated tag list to categorize (e.g. `--tags "auth,middleware"`)
- `--repo` is auto-injected by the wrapper.

comemory **auto-warns on near-duplicates**: if a similar memory already exists, it prints a warning (and emits a `duplicate_of` id with `--json`) but the save still proceeds. To replace an outdated memory instead of duplicating it, pass `--supersedes <id>` — the older memory is demoted in ranking and annotated `superseded_by` in search results.

### Search memories
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" search "<query>" --kind KIND
```
Query-driven recall — you must supply a natural-language `<query>`. Returns compact ranked results scoped to the current repo. comemory's default candidate window is 12; pass `--k N` to widen it, and `--kind` to filter by memory kind.

### Browse memories
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" list --kind KIND
```
Lists the current repo's memories (optionally filtered by `--kind`). Use to browse what's stored when you don't have a specific query.

### Statistics / health
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" stats
```
Reports data-directory + index health (maps to `comemory doctor`).

### Headline context lookup
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" context "<query>" --k N
```
Repo-scoped headline lookup — bundles the matching code symbol(s) with the
memories for a key (symbol name, file fragment, or phrase). Auto-injects `--repo`
like `search`.

### Delete a memory
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" delete <id>
```
Soft-deletes the 8-hex memory id (moves it to `.trash/`). The id is global, so
no `--repo` is needed. Prefer `--supersedes` on a new `save` when you are
*replacing* an outdated memory rather than removing it outright.

### First-time setup
Run `/comemory:setup` once per machine/repo. It detect-and-guides the `comemory`
binary (printing `brew install Falconiere/tap/comemory` if it is absent — the CLI
is **not** on crates.io), then wires the data dir, git hooks that auto-refresh the
code index on commit/merge/checkout, an initial `index-code`, and a completions
hint. Agents can trigger the same flow with `comemory.sh setup`. As of comemory
0.9.0 those git hooks also drive **auto-reinforcement** — commits touching a
memory's referenced files reinforce it with no manual `feedback`.

## Retrieval-quality loop (autonomous)
The loop verbs (`feedback`, `mine`, `tune`, `eval`, `prune`, `gc`, `rebuild`, `maintain`) are **LOCAL and token-free** — no LLM, no API, no `--repo`. They run **automatically once per day via the toolu SessionEnd hook**, so you rarely invoke them by hand.

The one verb you SHOULD call yourself: after you actually **use** a recalled memory, close the loop so future recall sharpens.
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" feedback <query_id> --used <id>
```
- `<query_id>` comes from a prior `search --json` envelope (run search with `--json` to get it).
- `--used <csv ids>` = memories that helped; `--irrelevant <csv ids>` = memories that didn't.
- This is the only loop step worth doing in-session; the rest are handled by the daily hook.

## 1. Memory Before Files — Layered Recall

Before exploring the codebase to *understand* something, search **comemory first**, then fall back to **ast-grep** for structural patterns and **Grep** for exact literals. Never jump straight to Read/Grep/Glob for *understanding* (as opposed to known-file reads).

State the repo scope before searching. All commands below use the auto-scoping wrapper.

```
Need to understand something?
│
├─ Architecture/structure question
│   ├─ comemory.sh search "architecture <module>" → past decisions
│   ├─ Hit  → use it
│   └─ Miss → ast-grep on relevant declarations, then Grep on keywords.
│              Save findings back via `comemory.sh save … --kind decision`.
│
├─ Where is the code for X?
│   ├─ comemory.sh search "file-map <area>" → cached path
│   ├─ Hit  → go directly to likely files, verify
│   └─ Miss → ast-grep for the call/def shape, or Grep for a literal name.
│              Save mapping via `comemory.sh save … --kind discovery`.
│
├─ How does pattern X work?
│   ├─ comemory.sh search "pattern <name>"
│   ├─ Hit  → use/validate
│   └─ Miss → ast-grep for the pattern shape, Read the hits, then save
│              (--kind pattern).
│
├─ What calls / what does Y call?
│   └─ ast-grep for the call shape (e.g. `$_.Y($$$)`) or Grep for the symbol.
│
└─ What was decided about X?
    ├─ comemory.sh search "decision <topic>"
    ├─ Hit  → reference and verify in docs/git if needed
    └─ Miss → check docs/git, then save (--kind decision)
```

**Skip memory + structural search** (go straight to files) when:
- User explicitly says "read this file" or asks to edit a specific file
- Running tests, builds, or git commands (execution, not knowledge)
- Checking current state (git status/diff, file contents you need to modify)
- Exact text/import search → use Grep
- Path/filename pattern → use Glob

**Self-check**: *"Am I about to read files just to understand something I might already know from a previous session, or that ast-grep / Grep could surface in one shot?"*

See also: `skills/ast-grep/SKILL.md` for structural search and rewrite.

## 2. Save What You Learn
After any exploration that yields **reusable knowledge**, save (announce the scope first):
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" save "<title>" "<body>" --kind KIND --tags "category,key"
```
### When to save (mandatory)
- Architecture or design decision made
- File purpose/location discovered during exploration
- Pattern documented or identified
- Bug fixed (include root cause)
- Convention established or clarified
- Gotcha, nuance, edge case, or unexpected behavior found
- User preference or constraint learned
- Configuration change or environment setup
- Quality gate/e2e failure discovered (save failing command + symptom)
- Quality gate/e2e fix validated (save what changed and why it worked)
### Self-check after EVERY task
> "Did I just make a decision, fix a bug, learn something non-obvious, or establish a convention? If yes → save NOW."
> "Did I just hit/fix a quality or e2e gate? If yes → save NOW."
> "Did I name the repo scope before calling comemory?"
### Kind + tags convention
Pick the `--kind` that matches the memory, and use `--tags` to add a searchable category:

| Memory | `--kind` | Example tags |
|---|---|---|
| Architecture | `decision` | `architecture,api-routing` |
| File map | `discovery` | `file-map,pipeline-queries` |
| Pattern | `pattern` | `query-hooks` |
| Decision | `decision` | `state-management` |
| Bug fix | `bug` | `canvas-drag-offset` |
| Convention | `convention` | `import-aliases` |
| Gotcha | `note` | `gotcha,orpc-client-types` |

If a memory updates an outdated one, pass `--supersedes <id>` to replace it (comemory also auto-warns when it detects a near-duplicate).
### Content format
```
**What**: One sentence — what was done/learned
**Why**: What motivated it
**Where**: Files or paths affected
**Learned**: Gotchas, edge cases (omit if none)
```
### Example save (with explicit scope announcement)
> Scoping comemory to **toolu** for save: decision / auth-middleware.
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" save "JWT auth middleware" "**What**: Added JWT validation middleware\n**Why**: API routes needed authentication\n**Where**: src/middleware/auth.ts\n**Learned**: Must set httpOnly flag on cookies" --kind decision --tags "auth,middleware"
```

## 3. Search Protocol (Progressive Disclosure)
Don't dump everything. Start narrow:
```
1. comemory.sh search "keywords"        → ranked candidates (auto-scoped)
2. comemory.sh list --kind KIND         → browse a kind if the query came up short
```
Start at layer 1. Only go to `list` if a targeted query isn't enough.
**When to search:**
- User asks to recall anything ("remember", "what did we do", "acordate", "que hicimos")
- Starting work on something that might overlap past sessions
- User mentions a topic you have no context on
- Before exploring files to understand architecture/patterns

## 4. Session Lifecycle
### Session start (recommended)
At the start of a session, announce the repo scope, then recall with a query for whatever you're about to work on:
```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" search "<what you're about to work on>"
```
### Realtime saves (mandatory)
Save learnings immediately as they happen — after every decision, bugfix, discovery, or pattern. Do NOT defer to session end. Announce scope before each save.
### Post-compaction recovery
If you see a compaction message or "FIRST ACTION REQUIRED":
1. Announce the repo scope.
2. Call `"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory/comemory.sh" search "<current task / open thread>"` to recover relevant prior context.
3. Only THEN continue working.

## 5. Raw CLI Fallback (rare)
Only use raw `comemory` directly when the wrapper is unavailable AND you can pass `--repo` explicitly. The `comemory-scope` pre-tool hook denies raw `search`/`save` calls missing `--repo`. Format:
```bash
comemory <subcmd> … --repo <repo-name>
```
Prefer the wrapper.
