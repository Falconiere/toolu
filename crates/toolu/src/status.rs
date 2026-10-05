//! `toolu status`: the status snapshot, not ported yet (#445).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "status",
  about: "Show the repository, gate and plugin status",
  verbs: &[],
  issues: &[445],
};

/// The `toolu status` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu status` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/status_test.rs"]
mod tests;
