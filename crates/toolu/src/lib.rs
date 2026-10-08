//! The toolu plugin's crate, the hub: the toolu namespaces, the tool hooks
//! (`toolu hook pre-tools` and `post-tools`, which `crates/cli` routes here) and
//! the rule crates, which only the hub may link (#460), re-exported so
//! `crates/cli` can register their namespaces.

use std::path::PathBuf;

use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;

/// The standalone delegation model-tier hook.
pub mod agent_tier;
/// `toolu config`.
pub mod config;
/// `toolu debug`.
pub mod debug;
/// `toolu doctor`.
pub mod doctor;
/// `toolu ledger`.
pub mod ledger;
/// The standalone MCP blocker hook.
pub mod mcp_hook;
/// `toolu serve`.
pub mod serve;
/// `toolu setup`.
pub mod setup;
/// `toolu status`.
pub mod status;
/// `toolu hook pre-tools` and `toolu hook post-tools`.
pub mod tool_hook;

pub use toolu_ast_grep as ast_grep;
pub use toolu_python_quality as python_quality;
pub use toolu_rust_quality as rust_quality;
pub use toolu_ts_quality as ts_quality;

/// The plugin this crate belongs to: `plugins/toolu`.
pub const PLUGIN: &str = "toolu";

/// Roots for `ctx`, with `--config-dir` overlaid, and the working directory.
pub(crate) fn overlaid_roots(ctx: &Ctx) -> (Roots, PathBuf) {
  let mut env = Env::process();
  if let Some(dir) = ctx.config_dir.as_deref().and_then(|dir| dir.to_str()) {
    env = env.with("TOOLU_CONFIG_DIR", dir);
  }
  let cwd = current_dir().unwrap_or_else(|_| PathBuf::from("."));
  (Roots::new(env, ctx.host), cwd)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
