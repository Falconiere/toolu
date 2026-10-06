//! The host a command runs under and where that host keeps its files
//! (`packages/toolu-core/src/host`): detection, the config, project, state and
//! plugin roots, the host-native invocation and install commands, and the Codex
//! plugin snapshot. Every function reads an explicit [`crate::env::Env`].

pub mod detect;
pub mod roots;
pub mod snapshot;
