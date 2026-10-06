//! The toolu-review plugin's crate: the `toolu review` namespace, not ported yet (#432).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/toolu-review`.
pub const PLUGIN: &str = "toolu-review";

const NAMESPACE: Planned = Planned {
  name: "review",
  about: "Pre-push code review that mirrors the CI review bot",
  verbs: &["write-state", "status"],
  issues: &[432],
};

/// The `toolu review` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu review` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
