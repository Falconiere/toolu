//! Shared host/plugin hook contracts, payloads, native launchers and CLI exits.
//! This protocol layer depends only on `serde` and `serde_json`.

/// Gate decisions and precedence.
pub mod decision;
/// Host-specific hook output encoding.
pub mod encode;
/// Shared hook event names.
pub mod event;
/// Native CLI exit codes.
pub mod exit;
/// Hook entrypoint and panic boundary.
pub mod hook;
/// Supported coding hosts.
pub mod host;
/// Native binary installation guidance.
pub mod install;
/// Generated native host launchers.
pub mod launcher;
/// Host-native event mappings.
pub mod native;
/// Open payload normalization.
pub mod normalize;
/// Normalized hook event contract.
pub mod normalized;
/// OpenCode payload bridge.
pub mod opencode;
/// Host input payloads.
pub mod payload;
/// Bounded hook stdin reading.
pub mod stdin;
/// Shared protocol text helpers.
pub mod text;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "protocol";

/// The integer hook-interface version compiled into the binary (#411). Each
/// `plugin.json` declares the one it was written for as `hookProtocol`; it is
/// bumped only when the `toolu hook` contract or a documented verb breaks.
pub const HOOK_PROTOCOL: u32 = 3;
