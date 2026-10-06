//! `toolu debug`: the debugging helpers of the toolu plugin's debug skill, not ported yet (#425).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "debug",
  about: "Debugging helpers: I/O capture, logs, stacks and failing tests",
  verbs: &["io", "log", "stack", "testfail"],
  issues: &[425],
};

/// The `toolu debug` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu debug` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/debug_test.rs"]
mod tests;
