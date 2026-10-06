//! What the small `SessionStart` hooks share (`packages/toolu-core/src/startup`):
//! publishing a plugin file at a stable config-root path, bounded context
//! output, Codex plugin-dependency warnings with host-native install commands,
//! and the startup report a self-hosting bootstrap reads (`TOOLU_STARTUP_REPORT`).

pub mod context;
pub mod dependencies;
pub mod publish;
pub mod report;
