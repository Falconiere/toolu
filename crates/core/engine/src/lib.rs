//! Shared hook dispatch, gate state, and delivery primitives for Toolu plugins.

use std::fmt;

/// PR babysitter interface used by the epic engine.
pub mod babysit;
/// Built-in hook gate tables.
pub mod builtins;
/// Git operations identified in parsed shell commands.
pub mod detect;
/// Pre-tool and post-tool hook dispatch.
pub mod dispatch;
/// Documentation review registration and results.
pub mod docs_review;
/// Shared interface for native hook gates.
pub mod gate;
mod gates;
/// State cleanup and idle checks.
pub mod gc;
/// Delivery plan ledger and verification.
pub mod ledger;
/// Structural summaries of source files.
pub mod outline;
/// Language quality rules and findings.
pub mod quality;
/// Installed hook rule discovery.
pub mod registry;
/// Resource leases and managed jobs.
pub mod resources;
/// Push-review state and validation.
pub mod review_state;
/// Status snapshots shared with the hub.
pub mod status;
/// Hook dispatch traces.
pub mod trace;
/// Delivery gate verdicts.
pub mod verdict;
/// Waiver handling for push checks.
pub mod waiver;

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
  /// GitHub asked the owning plugin to pause requests.
  RateLimited {
    /// Delay supplied by `Retry-After`, when present.
    retry_after: Option<std::time::Duration>,
    /// Latest GraphQL primary capacity, when present.
    remaining: Option<u64>,
    /// Primary reset as Unix seconds, when present.
    reset_at: Option<u64>,
  },
}

impl fmt::Display for LinkError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotPorted { issue } => write!(f, "not ported yet (#{issue})"),
      Self::Failed(message) => f.write_str(message),
      Self::RateLimited { .. } => f.write_str("GitHub rate limited the tick"),
    }
  }
}

impl std::error::Error for LinkError {}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
