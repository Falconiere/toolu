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

pub mod apply_patch;
pub mod ctx;
pub mod diff_sha;
pub mod edit_records;
mod gate_doc;
pub mod gate_file;
pub mod gate_schema;
pub mod git;
pub mod io;
mod js_order;
pub mod lock;
mod sweep_telemetry;
pub mod sweeper;
pub mod telemetry;
pub mod telemetry_schema;
pub mod time;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "state";
