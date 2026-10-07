//! toolu core, `engine` layer (#418): the `PreToolUse` and `PostToolUse` walk of
//! `@toolu/core/dispatch` and `@toolu/core/registry`.
//!
//! - `dispatch`: `dispatch_pre_tool` and `dispatch_post_tool`, the walk over one
//!   hook call: built-in gates (`gate`, `builtins`), then the registry; deny over
//!   ask over advisory before a tool, block over advisory after it, and one walk
//!   per path of a multi-file patch;
//! - `registry`: the `<config>/toolu/{pre,post}-tools.d` listing, installed-plugin
//!   gating and the Codex prune; manifests enable compiled-in rules, `.sh` modules
//!   run as executables and `.js` modules through a temporary Bun bridge;
//! - `trace`: what a dispatch did with each module;
//! - `detect`: the git operations a parsed command performs (push, commit, the
//!   pushed repository and branch).

pub mod builtins;
pub mod detect;
pub mod dispatch;
pub mod gate;
pub mod registry;
pub mod trace;

pub use dispatch::{
  DispatchOptions, Dispatched, ModuleResult, Phase, dispatch_post_tool, dispatch_pre_tool,
};

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "engine";
