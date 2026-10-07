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
//!
//! Cross-plugin traits (#460): plugin logic one plugin owns and another uses
//! is a trait here. The owner implements it and `crates/cli` passes that
//! implementation to the user, so no plugin crate depends on another:
//!
//! - [`babysit::BabysitTick`]: owned by pr-babysit, run by the epic engine;
//! - [`status::StatusSnapshot`]: owned by the hub (`crates/toolu`), rendered by
//!   statusline.

use std::fmt;

pub mod babysit;
pub mod builtins;
pub mod detect;
pub mod dispatch;
pub mod gate;
pub mod registry;
pub mod status;
pub mod trace;

pub use dispatch::{
  DispatchOptions, Dispatched, ModuleResult, Phase, dispatch_post_tool, dispatch_pre_tool,
};

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "engine";

/// Why a cross-plugin call gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
  /// The owning plugin has not ported this logic to Rust yet.
  NotPorted {
    /// The issue that ports it.
    issue: u32,
  },
  /// The owner ran and failed; the message is fit to show a user.
  Failed(String),
}

impl fmt::Display for LinkError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotPorted { issue } => write!(f, "not ported yet (#{issue})"),
      Self::Failed(message) => f.write_str(message),
    }
  }
}

impl std::error::Error for LinkError {}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
