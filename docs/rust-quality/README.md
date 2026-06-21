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
| File line limit | 250 lines | `lang.rust.maxFileLines` in toolu.config.json |
| Function line limit | 80 lines | `lang.rust.maxFnLines` |
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

### 7. Module Structure

```rust
// ❌ BANNED — a mod.rs that carries logic
// src/auth/mod.rs
pub mod token;
pub fn validate(t: &str) -> bool { /* ... */ }   // logic belongs in a submodule

// ✅ CORRECT — mod.rs is wiring only
// src/auth/mod.rs
//! Auth module.
pub mod token;
pub use self::token::validate;
```

- **`mod.rs`-no-logic** — a `mod.rs` may contain only `mod` declarations, `pub use` re-exports, doc comments, and attributes. Any item (`fn`/`struct`/`impl`/`const`/…) blocks. Toggle: `lang.rust.modRsNoLogic` (default on).
- **Generic file names** — `utils.rs`, `helpers.rs`, `common.rs`, `misc.rs` are banned; rename to a name that states the file's single responsibility. Files directly under a `shared/` or `common/` directory are exempt. The forbidden list is `lang.rust.forbiddenFileNames` (default `utils,helpers,common,misc`).

### 8. Module Docs

```rust
// ✅ CORRECT — //! header with a documented surface
//! Token validation.
//!
//! # Public API
//! - `validate` — verify a JWT and return its claims.
```

Every `mod.rs` / `lib.rs` / `main.rs` and every non-test `src/*.rs` file must open with a module-level `//!` doc header. A **missing header blocks**; a header that lacks a `# Public API` or `# Usage` section is **advisory** (case-insensitive). Toggle: `lang.rust.requireModuleDoc` (default on).

### 9. Layering

A child module file (a `src/*.rs` that is not `mod.rs`/`lib.rs`/`main.rs`) is checked for two leaks, **best-effort heuristics, not full visibility resolution**:

- A top-level item declared bare `pub` — prefer `pub(super)` / `pub(crate)` unless it is part of the crate's public API. (`pub(...)` restricted forms are left alone.) **Blocks.**
- A `use crate::…` that reaches several segments deep into a sibling's internals. **Blocks** per-file; the repo-wide scanner adds a cross-file check over `crate::`/`super::` imports (does the reached item appear in the sibling `mod.rs`'s `pub use` surface?) that is **advisory only** and conservatively skips anything ambiguous (glob/renamed re-exports, no declared surface). Toggle: `lang.rust.enforceLayering` (default on).

These structural rules live in the canonical `rust-rules.sh` rule library, shared verbatim by the per-edit gate and the repo-wide scanner (see [`/rust-refactor`](#rust-refactor) below).

## How the Gate Works

1. **Agent edits a `.rs` file** — `Write` or `Edit` tool call
2. **PostToolUse hook fires** — the assembled module checks the file
3. **Violation found** → gate goes **failing**, new task blocked until fixed
4. **Fix the violation** → gate clears, continue working

The gate is **multi-slot**: a failing test command and a failing file check are tracked independently — fixing one never silently masks the other.

The gate keys on the **nearest** enclosing `Cargo.toml` walking up from the edited file, so a crate in a nested workspace (e.g. `packages/backend/`) is covered — not just one at the repo root.

## Configuration

Configure thresholds per project in `toolu.config.json`:

```json
{
  "version": 1,
  "lang": {
    "rust": {
      "maxFileLines": 400,
      "maxFnLines": 100,
      "maxImplLines": 250,
      "forbiddenFileNames": ["utils", "helpers", "common", "misc"],
      "modRsNoLogic": true,
      "requireModuleDoc": true,
      "enforceLayering": true
    }
  }
}
```

Precedence for the numeric thresholds: project/user override → built-in default (250/80/200). A value of `0` or `"off"` means "no override" and falls through. The structural toggles (`modRsNoLogic`, `requireModuleDoc`, `enforceLayering`) default to on; `forbiddenFileNames` defaults to `utils,helpers,common,misc`.

## Usage Example

```text
# Session with the rust-quality plugin installed:

User: "Add a config validation function to src/config.rs"
Agent: *writes the function*

> PostToolUse: Checking src/config.rs...
> ❌ Gate: FAILING
>   - src/config.rs:142: .unwrap() on Result — use ? or match
>   - src/config.rs:45: Function exceeds 80-line limit (94 lines) — extract helpers
>   - src/config.rs:1: File exceeds 250-line limit (312 code lines) — split into submodules

# Agent is BLOCKED from starting new tasks until these are fixed:
#   - Replace .unwrap() with ?
#   - Split validate_config into smaller functions
#   - Extract helper module to reduce file size

> PostToolUse: Checking src/config.rs...
> ✅ Gate: PASSING
```

## `/rust-refactor`

The per-edit gate keeps a crate *clean as it grows*. `/rust-refactor` brings an **existing** Cargo workspace up to these rules in one pass. The workspace need not be at the repo root (a nested `packages/backend/` layout resolves). It always runs **report → confirm → apply** and never auto-applies:

1. **Preflight** — abort on a dirty tree, a non-Cargo, or a non-git target; on success, work lands on a `rust-quality/refactor` branch.
2. **Audit** (read-only) — `rust-scan.sh --json` reports structural violations (the same `rust-rules.sh` rules the gate runs, plus the cross-file advisory layering check) and which of `rustfmt.toml` / `clippy.toml` / `deny.toml` / `[workspace.lints]` are present or missing; `cargo fmt --check` and `cargo clippy` are captured but not fixed. Findings are grouped by autofix class (`fmt`, `clippy-fix`, `config`, `restructure`/`split`/`rename`/`move`, `manual`).
3. **Confirm** — you approve all classes or a subset. `config` (scaffold) must run before `clippy-fix`, since the deny lints define what `clippy --fix` repairs; declining `config` while accepting `clippy-fix` is rejected.
4. **Apply** — mechanical classes (`config`, `fmt`, `clippy-fix`) run via the scaffold + fix scripts; each step is committed, `cargo check`'d, and rolled back on failure. Structural classes are **agent-driven** (split oversized files into `dir/mod.rs` folder modules, extract inline tests, flatten nested tests, rename generic files, add `//!` docs, tighten visibility) — each step is likewise `cargo check`-gated and `git reset --hard` rolled back if it breaks the build, recorded as a `manual` residual. Files with non-standard module mapping (`#[path = …]`, `include!()`, macro-generated modules) are skipped as `manual`.
5. **Report** — a diff summary of what landed plus the `manual` residuals for you to finish by hand.

### Tools

Both are sourced/driven by the command and can be run standalone:

- **`scripts/rust-scan.sh`** `[--path <dir>] [--json] [--staged]` — the repo-wide structural scanner. It resolves the workspace, runs every `rust-rules.sh` rule over each `.rs` file plus the per-crate cross-file layering check, and emits a JSON or human report. It **sources** `rust-rules.sh` so the scan and the per-edit gate share one rule definition with zero drift.
- **`scripts/rust-scaffold.sh`** `--path <repo>` — the deterministic config scaffolder. It writes `rustfmt.toml` / `clippy.toml` / `deny.toml`, a `lefthook.yml` rust pre-commit hook, and a `.github/workflows/rust.yml`, then propagates `[workspace.lints]` into the root manifest and opts every member crate in with `[lints] workspace = true`. It is **merge-not-clobber** and idempotent: absent files are copied, present files gain only the keys/tables they lack, and existing values/comments/ordering are preserved verbatim. Templates live in `plugins/rust-quality/templates/`.

Repo-wide error-handling / `unsafe` / lint-suppression enforcement is owned by the scaffolded `cargo clippy` denies (live via `[workspace.lints]`); the per-edit gate keeps a fast ast-grep subset of those checks for instant edit-time feedback.

## Hooks

The fragments register into the core toolu dispatcher and run only while this plugin is installed. Uninstalling immediately removes the Rust rules:

```text
/plugin uninstall rust-quality@toolu
# → All Rust checks stop firing on the next edit
```
