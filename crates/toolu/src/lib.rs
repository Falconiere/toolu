//! The toolu plugin's crate, the hub: the toolu namespaces (`hook` lives in
//! `crates/cli` until #418) and the rule crates, which only the hub may link
//! (#460), re-exported so `crates/cli` can register their namespaces.

/// `toolu config`.
pub mod config;
/// `toolu debug`.
pub mod debug;
/// `toolu doctor`.
pub mod doctor;
/// `toolu ledger`.
pub mod ledger;
/// `toolu serve`.
pub mod serve;
/// `toolu setup`.
pub mod setup;
/// `toolu status`.
pub mod status;

pub use toolu_ast_grep as ast_grep;
pub use toolu_python_quality as python_quality;
pub use toolu_rust_quality as rust_quality;
pub use toolu_ts_quality as ts_quality;

/// The plugin this crate belongs to: `plugins/toolu`.
pub const PLUGIN: &str = "toolu";

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
