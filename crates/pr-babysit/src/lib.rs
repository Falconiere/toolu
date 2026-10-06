//! The pr-babysit plugin's crate: the `toolu babysit` namespace, not ported yet (#433).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/pr-babysit`.
pub const PLUGIN: &str = "pr-babysit";

const NAMESPACE: Planned = Planned {
  name: "babysit",
  about: "Babysit a PR until CI, review threads and the review-bot verdict are clear",
  verbs: &[
    "tick",
    "collect",
    "record",
    "reply",
    "resolve",
    "route-fix",
    "dispatch-fix",
  ],
  issues: &[433],
};

/// The `toolu babysit` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu babysit` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
