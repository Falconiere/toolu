---
name: ast-grep
description: "ALWAYS ACTIVE — Structural code search and rewrite protocol. You MUST use ast-grep instead of Grep/sed for any 'find/rewrite by code shape' task (AST patterns, not text). Falls back to Grep for exact literals only."
---

# ast-grep Structural Search & Rewrite Protocol

You have ast-grep (tree-sitter AST pattern matcher + rewriter) via the `ast-grep` / `sg` CLI.
This protocol is **MANDATORY and ALWAYS ACTIVE**.

## Hard Constraints

- Use `ast-grep run -p '<pattern>'` BEFORE Grep when the query is about **code shape** (AST), not text.
- Use `ast-grep run -p '...' -r '...'` BEFORE sed/Edit-loops when rewriting many call sites.
- Always pass `-l <lang>` for clarity (`typescript`, `tsx`, `javascript`, `rust`, `python`, `go`, ...).
- Never edit hand-rolled regex over multi-line code — use ast-grep instead.

## Position in the Search Stack

```
comemory → "what was decided / where is X" (memory)
ast-grep → "find / rewrite this code shape" (AST structural)   ← here
Grep     → "find exact literal `FOO_BAR`"                     (text)
Glob     → "list paths matching `**/*.test.ts`"               (paths)
Read     → "open known file"                                   (verify)
```

## When to Use ast-grep

✓ "Find all `useState($X)` calls without initial value"
✓ "Find all `try` blocks without `catch`"
✓ "Replace `console.log($X)` with `logger.info($X)` repo-wide"
✓ "Find all functions returning `Promise<void>`"
✓ "Find all `export default function` declarations"
✓ "Find all `await fetch($URL)` without error handling"
✓ Bulk safe refactor across many files (AST-aware, comment-preserving)

## When NOT to Use ast-grep

✗ Semantic intent ("how does auth work") → check comemory memory, then Grep on keywords + Read on hits
✗ Exact literal in config files (`*.toml`, `*.md`, `*.json`) → Grep
✗ Path/filename patterns → Glob
✗ Single-file targeted edit → Read + Edit

## CLI Reference

### Search
```bash
ast-grep run -p '<pattern>' -l <lang> [paths...]
ast-grep run -p '<pattern>' -l typescript apps/api/src
```

### Rewrite (preview)
```bash
ast-grep run -p '<pattern>' -r '<replacement>' -l <lang> [paths...]
```
Shows diff. Add `-U` (or `--update-all`) to apply.

### Apply rewrite
```bash
ast-grep run -p '<pattern>' -r '<replacement>' -l <lang> -U [paths...]
```

### Inspect AST (debugging patterns)
```bash
ast-grep run -p '<pattern>' -l <lang> --debug-query=ast
```

### Strictness
```bash
--strictness smart      # default — ignores trivia
--strictness relaxed    # ignores comments
--strictness signature  # ignores text — match by shape only
```

## Pattern Syntax (Quick)

- `$X` — single named metavariable (any node)
- `$$$ARGS` — multi-node metavariable (zero or more)
- `$_` — anonymous wildcard
- Same metavariable name = same content (linear pattern)

Examples:
| Want | Pattern |
|------|---------|
| any `console.log` call | `console.log($$$)` |
| `useState` with no arg | `useState()` |
| same var assigned to itself | `$X = $X` |
| arrow fn with single body expr | `($$$) => $BODY` |
| catch-all `try` no catch | `try { $$$ } finally { $$$ }` |

## Workflow — Search

1. Draft pattern. Run with `--debug-query=ast` once to verify shape.
2. Run real search: `ast-grep run -p '...' -l <lang> <paths>`.
3. If too many hits → narrow with `--strictness smart` or extra context nodes.
4. If zero hits → broaden to `$_` / `$$$`, recheck AST.

## Workflow — Rewrite

1. **Preview first**: never use `-U` until diff looks right.
2. Run preview: `ast-grep run -p '...' -r '...' -l <lang> <paths>`.
3. Read the diff. Confirm metavariables map correctly.
4. Apply: same command with `-U`.
5. Run your project's typecheck/lint (use the detected package manager — see `detect_node_pm` / `detect_rust` in `hooks/lib/detect.sh`) to verify zero regressions.
6. Save the pattern if reusable — when the `comemory` plugin is installed, persist it via its wrapper: `comemory.sh save "ast-grep <name> pattern" "<pattern + what it matches>" --kind pattern --tags "ast-grep"`.

## Self-Check Before Editing

> "Am I about to write a regex over multi-line code? → ast-grep instead."
> "Am I about to Edit-loop the same change across many files? → ast-grep rewrite instead."
> "Am I searching for code by shape (call form, signature, control flow)? → ast-grep, not Grep."

## Config (sgconfig.yml — optional, lazy)

If a recurring rule emerges (e.g. forbid `console.log` in apps/api), promote it to `sgconfig.yml`:
```bash
ast-grep new project   # bootstrap config
ast-grep new rule      # add reusable rule
ast-grep scan          # run all configured rules
```
Hold off until pattern is proven across ≥3 files.

## Cross-Reference

- Memory: `skills/agent-memory/SKILL.md`
- Project rules: `AGENTS.md`, `CLAUDE.md`, and other host instruction files
