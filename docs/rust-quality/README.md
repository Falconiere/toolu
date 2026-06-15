# rust-quality — Rust Quality Gate

**Type:** Quality Gate | **Version:** 0.1.0 | **Depends on:** `toolu`

Rust `PostToolUse` quality checks registered into the toolu hook engine. Every Rust file the agent edits is checked on the spot.

## Install

```text
/plugin install rust-quality@toolu
```

## What It Provides

### Post-Edit Quality Checks

Every Rust file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart` and run only while this plugin is installed — **uninstall it and the Rust rules vanish, fail-closed.**

## Checks Enforced

### 1. Size Discipline

| Limit | Default | Configurable via |
|-------|---------|-----------------|
| File line limit | 500 lines | `lang.rust.maxFileLines` in toolu.config.json |
| Function line limit | 50 lines | `lang.rust.maxFnLines` |
| `impl` block line limit | 200 lines | `lang.rust.maxImplLines` |

Line counting excludes blank lines and comments. If the scan ends mid-`/* */`, it falls back to raw line count rather than risk under-counting.

### 2. No Unwrap / Expect

```rust
// ❌ BANNED
let config = load_config().unwrap();
let config = load_config().expect("should be valid");

// ✅ CORRECT — propagate with ?
let config = load_config()?;

// ✅ CORRECT — match and handle
let config = match load_config() {
    Ok(c) => c,
    Err(e) => {
        tracing::error!("Failed to load config: {}", e);
        return Err(AppError::ConfigLoad(e));
    }
};
```

Every `.unwrap()` and `.expect()` in `src/` is a gate violation — use `?` or explicit `match`.

### 3. No Unsafe Blocks

```rust
// ❌ BANNED — unsafe block in src/
unsafe {
    let ptr = buffer.as_ptr().add(offset);
    ptr.read()
}

// ✅ IF TRULY NEEDED — must be registered in exemptions file
```

Register any unavoidable `unsafe` in `plugins/toolu/settings/rust-unsafe-exemptions.txt`.

### 4. No Lint Suppression

```rust
// ❌ BANNED
#[allow(dead_code)]
pub fn helper() { }

#[expect(unused_variables)]
fn process(data: Vec<u8>) { }

// ✅ CORRECT — fix the lint, don't suppress it
```

### 5. Test Layout

```rust
// ❌ BANNED — inline tests in src/
// src/auth.rs
#[cfg(test)]
mod tests {
    use super::*;
    // ...
}

// ✅ CORRECT — flat tests/ directory
// tests/auth_test.rs
use my_crate::auth;
// ...
```

Tests must live in a sibling `tests/` directory, kept flat (only `fixtures/`, `helpers/`, `common/` subdirectories).

### 6. Doc Comments on Public Items

```rust
// ✅ CORRECT — public item has a doc comment
/// Validates the incoming JWT token and returns the claims.
/// Returns `AuthError::Expired` if the token has timed out.
pub async fn validate_token(token: &str) -> Result<Claims, AuthError> {
    // ...
}

// ❌ BANNED — public function with no doc comment
pub fn validate_token(token: &str) -> Result<Claims, AuthError> {
    // ...
}
```

Every public item must carry a concise doc line.

## How the Gate Works

1. **Agent edits a `.rs` file** — `Write` or `Edit` tool call
2. **PostToolUse hook fires** — the assembled module checks the file
3. **Violation found** → gate goes **failing**, new task blocked until fixed
4. **Fix the violation** → gate clears, continue working

The gate is **multi-slot**: a failing test command and a failing file check are tracked independently — fixing one never silently masks the other.

## Configuration

Configure thresholds per project in `toolu.config.json`:

```json
{
  "version": 1,
  "lang": {
    "rust": {
      "maxFileLines": 600,
      "maxFnLines": 60,
      "maxImplLines": 250
    }
  }
}
```

Precedence: project/user override → built-in default (500/50/200). A value of `0` or `"off"` means "no override" and falls through.

## Usage Example

```text
# Session with the rust-quality plugin installed:

User: "Add a config validation function to src/config.rs"
Agent: *writes the function*

> PostToolUse: Checking src/config.rs...
> ❌ Gate: FAILING
>   - src/config.rs:142: .unwrap() on Result — use ? or match
>   - src/config.rs:45: function validate_config exceeds 50-line limit (72 lines)
>   - src/config.rs:1: file exceeds 500-line limit (543 lines)

# Agent is BLOCKED from starting new tasks until these are fixed:
#   - Replace .unwrap() with ?
#   - Split validate_config into smaller functions
#   - Extract helper module to reduce file size

> PostToolUse: Checking src/config.rs...
> ✅ Gate: PASSING
```

## Hooks

The fragments register into the core toolu dispatcher and run only while this plugin is installed. Uninstalling immediately removes the Rust rules:

```text
/plugin uninstall rust-quality@toolu
# → All Rust checks stop firing on the next edit
```
