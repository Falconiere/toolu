//! Native ast-grep registry rules and the `toolu ast-grep` command namespace.

mod cli;

mod launched;
mod nudge;
mod register;
mod report;
mod savings;

/// The search nudge rule enabled by its manifest.
pub use nudge::SEARCH_NUDGE;
/// The byte-savings rule enabled by its manifest.
pub use savings::BYTE_SAVINGS;

/// The `toolu ast-grep` namespace.
pub use cli::command;
/// Run a `toolu ast-grep` verb.
pub use cli::run;
/// Publish this plugin's native rule manifests at `SessionStart`.
pub use register::register;

/// The plugin this crate belongs to: `plugins/ast-grep`.
pub const PLUGIN: &str = "ast-grep";

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
