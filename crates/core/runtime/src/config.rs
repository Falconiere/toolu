//! `toolu.config.json` (`packages/toolu-core/src/config`): the loader with its
//! fail-closed envelope, the per-key resolvers that keep every jq fallback of
//! the bash libs, gate modes, quality thresholds, docs-sync globs, the one-time
//! Claude permissions write and the plugin settings files.

pub mod docs_sync;
/// Non-secret epic settings with forward-compatible nested keys.
pub mod epic;
/// Gate mode configuration.
pub mod gate_mode;
/// Configuration loading.
pub mod load;
/// The global pane-manager (multiplexer) selection.
pub mod multiplexer;
/// Host permission configuration.
pub mod permissions;
/// Quality rule configuration.
pub mod quality;
/// Read helpers for configuration files.
pub mod read;
mod redaction;
/// User-only epic credentials and output redaction.
pub mod secrets;
/// Host settings files.
pub mod settings;
