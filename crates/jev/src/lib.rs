//! The jev plugin's crate: the `toolu jev` namespace, not ported yet (#430).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/jev`.
pub const PLUGIN: &str = "jev";

const NAMESPACE: Planned = Planned {
  name: "jev",
  about: "Typed judgments from Jev: yes/no probabilities, choices and scores",
  verbs: &["noul", "choice", "score", "ask"],
  issues: &[430],
};

/// The `toolu jev` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu jev` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
