# ast-grep

Structural code search & rewrite (ast-grep): a skill, a wrapper, and a `PreToolUse` `Grep → ast-grep` nudge registered into the toolu hook engine.

## Install

```
/plugin install ast-grep@toolu
```

Standalone, no dependencies.

## What it provides

- **`ast-grep` skill** — an always-active protocol that mandates ast-grep (tree-sitter AST patterns) over Grep/sed for any find-or-rewrite-by-code-shape task, falling back to Grep only for exact literals.
- **`search-nudge` (`PreToolUse`)** — nudges Grep on code files, and `grep`/`rg` in Bash, toward the proper structural tool.
- **`byte-savings` (`PostToolUse`)** + `byte-savings-report.sh` — instrumentation for tokens saved by structural search.

## The ast-grep binary

The skill drives the `ast-grep` (a.k.a. `sg`) CLI — a tree-sitter AST pattern matcher and rewriter. Install it via `brew install ast-grep`, `cargo install ast-grep`, or `npm i -g @ast-grep/cli`. The plugin's hook modules register themselves into the core toolu dispatcher at `SessionStart` and run only while this plugin is installed.
