//! `toolu setup`: the agent-profile setup of the toolu plugin, not ported yet (#445).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "setup",
  about: "Preview, install or remove toolu's agent profiles",
  verbs: &["agents"],
  issues: &[445],
};

/// The `toolu setup` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu setup` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/setup_test.rs"]
mod tests;
