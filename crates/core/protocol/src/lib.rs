//! toolu core, `protocol` layer: the hook interface shared by every host and plugin.
//!
//! It holds the hook-interface version (`HOOK_PROTOCOL`, #411), which host events
//! enforce, the install commands, and the generated `hooks.json` launcher (#412).

pub mod event;
pub mod install;
pub mod launcher;
pub mod stdin;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "protocol";

/// The integer hook-interface version compiled into the binary (#411). Each
/// `plugin.json` declares the one it was written for as `hookProtocol`; it is
/// bumped only when the `toolu hook` contract or a documented verb breaks.
pub const HOOK_PROTOCOL: u32 = 1;
