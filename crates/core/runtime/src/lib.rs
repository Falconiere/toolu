//! toolu core, `runtime` layer: what a hook learns from its process and the
//! calling plugin: argv, the binary's path, the plugin manifest, and the
//! version-skew rule of #411.

pub mod install;
pub mod invocation;
pub mod manifest;
pub mod skew;
pub mod version;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "runtime";
