//! toolu core, `mux` layer: which pane manager runs worker agents, and how to
//! reach it. herdr is the tmux-based agent manager toolu has always driven;
//! plain tmux is the fallback that needs no extra install. This crate resolves
//! the configured [`Multiplexer`] to a concrete [`Backend`] and answers the
//! reachability questions the doctor and the engines ask.

pub mod backend;
/// Reachability probes for herdr and tmux.
pub mod detect;

pub use backend::{Backend, resolve};
pub use detect::{herdr_reachable, session_name, socket_path, tmux_available};
pub use toolu_runtime::config::multiplexer::Multiplexer;

/// This crate's layer in `tooling/conventions/guardrails/rust/layers.json`.
pub const LAYER: &str = "mux";
