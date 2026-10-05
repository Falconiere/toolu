//! `toolu serve`: the `--stdio` server behind the OpenCode shim, not ported yet (#437).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "serve",
  about: "Serve the OpenCode shim over standard I/O",
  verbs: &[],
  issues: &[437],
};

/// The `toolu serve` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu serve` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/serve_test.rs"]
mod tests;
