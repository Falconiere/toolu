//! `toolu config`: the toolu.config.json verbs, not ported yet (#445).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "config",
  about: "Read, change and validate toolu.config.json",
  verbs: &["get", "set", "validate"],
  issues: &[445],
};

/// The `toolu config` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu config` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/config_test.rs"]
mod tests;
