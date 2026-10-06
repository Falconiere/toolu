//! `toolu doctor`: the installation check, not ported yet (#445).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "doctor",
  about: "Check the toolu installation and name what is wrong",
  verbs: &[],
  issues: &[445],
};

/// The `toolu doctor` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu doctor` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/doctor_test.rs"]
mod tests;
