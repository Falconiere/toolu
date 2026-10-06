//! toolu core, `state` layer (#415): every on-disk state file, read and
//! written byte for byte as `@toolu/core/state` does, so TypeScript and Rust
//! hooks share them during the migration.
//!
//! - `gate_file`: the multi-slot gate file (strict v1), written atomically under
//!   the `<file>.lock` protocol of `lock`;
//! - `telemetry`: closed-schema JSONL events; `sweeper`: spent-state reclaim;
//! - `edit_records`: `Edit`, `Write`, `MultiEdit` and `apply_patch` payloads as records;
//! - `diff_sha`: the branch-diff hash; `git`: branch, worktree and origin facts
//!   read from `.git`; `detect`: project markers, linters, tools and line counts.

pub mod ctx;
pub mod git;
pub mod io;
pub mod lock;
pub mod time;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "state";
