//! The ast-grep rule crate: its rules and the `toolu ast-grep` namespace, not ported yet
//! (#429). `crates/cli` reaches it through the hub (`crates/toolu`).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/ast-grep`.
pub const PLUGIN: &str = "ast-grep";

const NAMESPACE: Planned = Planned {
  name: "ast-grep",
  about: "Structural code search and rewrite with ast-grep",
  verbs: &["search", "files", "scan", "debug", "savings"],
  issues: &[429],
};

/// The `toolu ast-grep` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu ast-grep` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
