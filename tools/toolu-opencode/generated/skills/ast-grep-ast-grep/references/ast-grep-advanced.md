# ast-grep Advanced Usage

## Complex Rules (inline YAML)

For patterns beyond simple `--pattern`, use `scan` with inline YAML:

```bash
plugins/ast-grep/hooks/dist/ast-grep.js scan 'id: find-async
language: rust
rule:
  kind: function_item
  has:
    pattern: async fn \$NAME
    stopBy: end'
```

## Critical Rules

- `stopBy: end` required in `has`/`inside` for deep matching — without it, only direct children match
- `scan` rules require `kind` field
- `--max-results` only works in `scan` — use `| head -N` for `search`
- Escape `$` as `\$` in inline-rules strings (not needed in `--pattern` with single quotes)

## Common Patterns

```bash
# Rust: impl blocks for a trait
plugins/ast-grep/hooks/dist/ast-grep.js search 'impl $TRAIT for $TYPE { $$$BODY }' --lang rust

# Rust: async functions
plugins/ast-grep/hooks/dist/ast-grep.js search 'async fn $NAME($$$ARGS)' --lang rust

# Rust: functions returning Result
plugins/ast-grep/hooks/dist/ast-grep.js search 'fn $NAME($$$ARGS) -> Result<$$$>' --lang rust

# TypeScript: console.log calls
plugins/ast-grep/hooks/dist/ast-grep.js search 'console.log($$$ARGS)' --lang typescript

# File paths only (no content)
plugins/ast-grep/hooks/dist/ast-grep.js files 'impl $TRAIT for $TYPE { $$$BODY }' --lang rust

# Debug pattern parsing
plugins/ast-grep/hooks/dist/ast-grep.js debug 'fn $NAME($$$ARGS)' --lang rust
```
