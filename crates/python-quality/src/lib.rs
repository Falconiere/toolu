//! The python-quality rule crate: its rules and the `toolu python-quality` namespace, not
//! ported yet (#427). `crates/cli` reaches it through the hub (`crates/toolu`).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/python-quality`.
pub const PLUGIN: &str = "python-quality";

const NAMESPACE: Planned = Planned {
  name: "python-quality",
  about: "Python quality rules that run after each edit",
  verbs: &[],
  issues: &[427],
};

/// The `toolu python-quality` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu python-quality` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
