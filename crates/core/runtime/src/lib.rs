//! toolu core, `runtime` layer: what a command learns from its process and the
//! calling plugin (argv, the binary's path, the plugin manifest, the version-skew
//! rule of #411), and what a `toolu` verb receives and returns (#442).

/// The global flags a verb sees and the outcome it returns.
pub mod cli;
pub mod install;
pub mod invocation;
pub mod manifest;
/// The placeholder and guide namespaces plugin crates declare before their port.
pub mod namespace;
pub mod skew;
pub mod version;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "runtime";
