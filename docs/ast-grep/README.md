# ast-grep — Structural Code Search & Rewrite

**Type:** Code Intel | **Version:** 4.5.0 | **Standalone** (no dependencies)

Structural code search and rewrite powered by **tree-sitter AST patterns** — a skill, a wrapper, and a `PreToolUse` `Grep → ast-grep` nudge registered into the toolu hook engine.

## Install

```text
/plugin install ast-grep@toolu
```

Also install the `ast-grep` CLI binary (a.k.a. `sg`):

```bash
brew install ast-grep
# or: cargo install ast-grep
# or: npm i -g @ast-grep/cli
```

## What It Provides

### 1. `ast-grep` Skill (Always Active)

Mandates ast-grep over Grep/sed for any find-or-rewrite-by-code-shape task. Falls back to Grep only for exact literals.

### 2. Search Nudge (`PreToolUse`)

Nudges Grep on code files, and `grep`/`rg` in Bash, toward the proper structural tool.

### 3. Byte-Savings (`PostToolUse`)

Instrumentation for tokens saved by structural search vs. raw text Grep.

## Usage Examples

### Search — Find Code by Shape

```bash
# Find all useState calls without initial value
ast-grep run -p 'useState()' -l typescript apps/

# Find all try blocks without catch
ast-grep run -p 'try { $$$ } finally { $$$ }' -l typescript src/

# Find all functions returning Promise<void>
ast-grep run -p 'async function $NAME($$$): Promise<void>' -l typescript

# Find all export default function declarations
ast-grep run -p 'export default function $NAME($$$) { $$$ }' -l tsx

# Find any console.log call
ast-grep run -p 'console.log($$$)' -l typescript

# Find await fetch without error handling
ast-grep run -p 'await fetch($$$)' -l typescript

# Find all impl blocks for a specific trait (Rust)
ast-grep run -p 'impl $TRAIT for $TYPE { $$$ }' -l rust .
```

### Rewrite — Bulk Safe Refactor

```bash
# Preview first — never use -U until the diff looks right
ast-grep run -p 'console.log($$$)' -r 'logger.info($$$)' -l typescript -U src/

# Replace all throw new Error('msg') with a custom error helper
ast-grep run -p "throw new Error($MSG)" -r "throw Errors.validation($MSG)" -l typescript

# Add return type annotation to all exported functions
ast-grep run -p 'export function $NAME($$$) { $$$ }' -r 'export function $NAME($$$): Response { $$$ }' -l tsx -U apps/
```

### Inspect the AST (Debug Patterns)

```bash
ast-grep run -p 'useState()' -l typescript --debug-query=ast
```

### Pattern Syntax Quick Reference

| Pattern Variable | Meaning |
|-----------------|--------|
| `$X` | Single named metavariable (any node) |
| `$$$ARGS` | Multi-node metavariable (zero or more) |
| `$_` | Anonymous wildcard |
| Same name = same content | `$X = $X` matches self-assignment |

| Search Goal | Pattern |
|------------|---------|
| any `console.log` call | `console.log($$$)` |
| `useState` with no arg | `useState()` |
| same var assigned to itself | `$X = $X` |
| arrow fn with single body expr | `($$$) => $BODY` |
| catch-all `try` no catch | `try { $$$ } finally { $$$ }` |

### Search Stack Position

```
comemory   → "what was decided / where is X" (memory)
ast-grep   → "find / rewrite this code shape" (AST structural)   ← HERE
Grep       → "find exact literal FOO_BAR"                       (text)
Glob       → "list paths matching **/*.test.ts"                 (paths)
Read       → "open known file"                                   (verify)
```

### Recommended Workflow

```bash
# 1. Draft pattern, verify shape
ast-grep run -p '...' -l typescript --debug-query=ast

# 2. Run real search
ast-grep run -p '...' -l typescript src/apps/

# 3. Too many hits → narrow context
ast-grep run -p 'async function $_($$$) { $$$ await fetch($$$) $$$ }' -l typescript

# 4. Zero hits → broaden with wildcards
ast-grep run -p '$_.$_($$$)' -l typescript

# 5. For rewrites: preview first, apply with -U, run typecheck/lint
ast-grep run -p '...' -r '...' -l typescript src/    # preview
ast-grep run -p '...' -r '...' -l typescript -U src/  # apply
```

### When NOT to Use ast-grep

- **Semantic intent** ("how does auth work?") → comemory, then Grep + Read
- **Exact literal in config files** (`*.toml`, `*.md`, `*.json`) → Grep
- **Path/filename patterns** → Glob
- **Single-file targeted edit** → Read + Edit

### Project-Wide Rules (sgconfig.yml)

When a pattern proves reusable across ≥3 files, promote it:

```bash
ast-grep new project   # bootstrap config
ast-grep new rule      # add reusable rule
ast-grep scan          # run all configured rules
```

## Self-Check Before Editing

> "Am I about to write a regex over multi-line code? → ast-grep instead."
> "Am I about to Edit-loop the same change across many files? → ast-grep rewrite instead."
> "Am I searching for code by shape (call form, signature, control flow)? → ast-grep, not Grep."

## Hooks

| Hook | Event | Purpose |
|------|-------|--------|
| `search-nudge` | PreToolUse | Steers `grep`/`rg` on code files toward ast-grep |
| `byte-savings` | PostToolUse | Tracks tokens saved by structural vs. text search |

Hooks register into the core toolu dispatcher at `SessionStart` — they run only while this plugin is installed.
