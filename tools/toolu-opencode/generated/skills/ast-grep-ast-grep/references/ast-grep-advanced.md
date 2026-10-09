# ast-grep Advanced Usage

## Complex Rules (inline YAML)

For patterns beyond simple `--pattern`, use `scan` with inline YAML:

```bash
toolu ast-grep scan 'id: find-async
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
ast-grep run --pattern 'impl $TRAIT for $TYPE { $$$BODY }' --lang rust

# Rust: async functions
ast-grep run --pattern 'async fn $NAME($$$ARGS)' --lang rust

# Rust: functions returning Result
ast-grep run --pattern 'fn $NAME($$$ARGS) -> Result<$$$>' --lang rust

# TypeScript: console.log calls
ast-grep run --pattern 'console.log($$$ARGS)' --lang typescript

# File paths only (no content)
ast-grep run --pattern 'impl $TRAIT for $TYPE { $$$BODY }' --files-with-matches --lang rust

# Debug pattern parsing
ast-grep run --pattern 'fn $NAME($$$ARGS)' --debug-query=pattern --lang rust
```
