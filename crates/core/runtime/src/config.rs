//! `toolu.config.json` (`packages/toolu-core/src/config`): the loader with its
//! fail-closed envelope, the per-key resolvers that keep every jq fallback of
//! the bash libs, gate modes, quality thresholds, docs-sync globs, the one-time
//! Claude permissions write and the plugin settings files.

pub mod docs_sync;
pub mod gate_mode;
pub mod load;
pub mod permissions;
pub mod quality;
pub mod read;
pub mod settings;
