//! toolu core, `runtime` layer: what a command learns from its process and the
//! calling plugin (argv, the binary's path, the plugin manifest, the version-skew
//! rule of #411), and what a `toolu` verb receives and returns (#442).

pub mod cli;
pub mod cli_args;
pub mod config;
pub mod env;
pub mod host;
pub mod install;
pub mod invocation;
pub mod json;
pub mod manifest;
pub mod namespace;
pub mod process;
pub mod skew;
pub mod version;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "runtime";
