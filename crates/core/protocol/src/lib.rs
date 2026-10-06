//! toolu core, `protocol` layer: the contract shared by every host and plugin.
//!
//! It holds the hook-interface version (`HOOK_PROTOCOL`, #411), which host events
//! enforce, the install commands, the generated `hooks.json` launcher (#412), and
//! the exit codes and host names of the `toolu` CLI (#442).

pub mod decision;
pub mod encode;
pub mod event;
pub mod exit;
pub mod host;
pub mod install;
pub mod launcher;
pub mod native;
pub mod normalize;
pub mod normalized;
pub mod payload;
pub mod stdin;
pub mod text;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "protocol";

/// The integer hook-interface version compiled into the binary (#411). Each
/// `plugin.json` declares the one it was written for as `hookProtocol`; it is
/// bumped only when the `toolu hook` contract or a documented verb breaks.
pub const HOOK_PROTOCOL: u32 = 1;
