# ast-grep

Structural code search and rewrite: a skill, native `toolu ast-grep` commands, and two compiled rules enabled by manifests.

## Install

**Prerequisite for wrapper commands and rules:** the native [`toolu` binary](../../docs/install.md).

```
/plugin install ast-grep@toolu
```

The skill remains usable without toolu. Install the native `toolu` binary for the wrapper commands and optional rules.

## What it provides

- **`ast-grep` skill** — an always-active protocol that mandates ast-grep (tree-sitter AST patterns) over Grep/sed for any find-or-rewrite-by-code-shape task, falling back to Grep only for exact literals.
- **`search-nudge` (`PreToolUse`)** — nudges a structural Grep pattern on code files, and `grep`/`rg` run in Bash to search files, toward the proper structural tool.
- **`byte-savings` (`PostToolUse`)** + `toolu ast-grep savings <ledger.jsonl>` — records the bytes each Read, Grep, Glob and ast-grep call returns, and sums a session's ledger. On OpenCode, an ast-grep result also carries the session's report ([`docs/opencode.md`](../../docs/opencode.md#ast-grep)).
- **`toolu ast-grep search|files|scan|debug`** — wraps the external binary with `--color never` and infers `--lang` from the first file argument.

## The ast-grep binary

The skill drives the external `ast-grep` (a.k.a. `sg`) CLI — a tree-sitter AST pattern matcher and rewriter. Install it via `brew install ast-grep`, `cargo install ast-grep`, or `npm i -g @ast-grep/cli`. SessionStart publishes native rule manifests into the toolu registry. The rule library runs in process only while this plugin is installed and toolu is available.
