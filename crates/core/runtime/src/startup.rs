//! What the small `SessionStart` hooks share: retiring the stable config-root
//! paths that plugins published before #440, bounded context
//! output, Codex plugin-dependency warnings with host-native install commands,
//! and the startup report a self-hosting bootstrap reads (`TOOLU_STARTUP_REPORT`).

pub mod context;
pub mod dependencies;
pub mod native;
pub mod report;
pub mod retire;

pub use native::{native_toolu_advice, native_toolu_hook_output};
