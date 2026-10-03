# ast-grep

Structural code search & rewrite (ast-grep): a skill, a wrapper, and a `PreToolUse` `Grep → ast-grep` nudge registered into the toolu hook engine.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install ast-grep@toolu
```

Standalone, no dependencies.

## What it provides

- **`ast-grep` skill** — an always-active protocol that mandates ast-grep (tree-sitter AST patterns) over Grep/sed for any find-or-rewrite-by-code-shape task, falling back to Grep only for exact literals.
- **`search-nudge` (`PreToolUse`)** — nudges a structural Grep pattern on code files, and `grep`/`rg` run in Bash to search files, toward the proper structural tool.
- **`byte-savings` (`PostToolUse`)** + `hooks/dist/byte-savings-report.js <ledger.jsonl>` — records the bytes each Read, Grep, Glob and ast-grep call returns, and sums a session's ledger. On OpenCode, an ast-grep result also carries the session's report ([`docs/opencode.md`](../../docs/opencode.md#ast-grep)).
- **`hooks/dist/ast-grep.js`** — a CLI wrapper (`search`, `files`, `scan`, `debug`) that bakes in `--color never` and infers `--lang` from the first file argument.

## The ast-grep binary

The skill drives the `ast-grep` (a.k.a. `sg`) CLI — a tree-sitter AST pattern matcher and rewriter. Install it via `brew install ast-grep`, `cargo install ast-grep`, or `npm i -g @ast-grep/cli`. The hook modules are TypeScript bundles: `hooks/dist/register.js` publishes them into the core toolu dispatcher at `SessionStart`, and they run in process only while this plugin is installed.
