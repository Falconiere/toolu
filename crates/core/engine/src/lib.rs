//! toolu core, `engine` layer (#418): the `PreToolUse` and `PostToolUse` walk of
//! `@toolu/core/dispatch` and `@toolu/core/registry`.
//!
//! - `registry`: the `<config>/toolu/{pre,post}-tools.d` listing, installed-plugin
//!   gating and the Codex prune.

pub mod registry;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "engine";
