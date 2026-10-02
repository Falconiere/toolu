# ast-grep Advanced Usage

## Complex Rules (inline YAML)

For patterns beyond simple `--pattern`, use `scan` with inline YAML:

```bash
mod.sh ast-grep scan 'id: find-async
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
mod.sh ast-grep search 'impl $TRAIT for $TYPE { $$$BODY }' --lang rust

# Rust: async functions
mod.sh ast-grep search 'async fn $NAME($$$ARGS)' --lang rust

# Rust: functions returning Result
mod.sh ast-grep search 'fn $NAME($$$ARGS) -> Result<$$$>' --lang rust

# TypeScript: console.log calls
mod.sh ast-grep search 'console.log($$$ARGS)' --lang typescript

# File paths only (no content)
mod.sh ast-grep files 'impl $TRAIT for $TYPE { $$$BODY }' --lang rust

# Debug pattern parsing
mod.sh ast-grep debug 'fn $NAME($$$ARGS)' --lang rust
```
