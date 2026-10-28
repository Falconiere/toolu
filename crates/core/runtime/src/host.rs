//! The host a command runs under and where that host keeps its files
//! (`packages/toolu-core/src/host`): detection, the config, project, state and
//! plugin roots, the host-native invocation and install commands, and the Codex
//! plugin snapshot. Every function reads an explicit [`crate::env::Env`].

/// Codex marketplace pin status from `config.toml`.
pub mod codex;
/// Cursor local-plugin marker paths.
pub mod cursor;
/// Host detection from the environment.
pub mod detect;
/// Muse config paths and sandbox readability probes.
pub mod muse;
/// Config, project, state and plugin roots.
pub mod roots;
/// Codex plugin inventory snapshot.
pub mod snapshot;
