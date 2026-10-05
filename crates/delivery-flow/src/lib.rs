//! The delivery-flow plugin's crate: `toolu delivery-flow`, the guide of a Markdown-only
//! plugin.

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The plugin this crate belongs to: `plugins/delivery-flow`.
pub const PLUGIN: &str = "delivery-flow";

const GUIDE: Guide = Guide {
  name: "delivery-flow",
  about: "Deliver a task end to end, from brainstorm to a babysat PR (a Markdown-only plugin)",
  text: "delivery-flow is a Markdown-only plugin: it has no verbs. Its one skill delivers a task \
   end to end: brainstorm, an approved spec and plan, real-data execution, the pull request \
   and the pr-babysit handoff.\n\nRun it as /delivery-flow:delivery-flow in Claude Code or \
   $delivery-flow:delivery-flow in Codex.",
};

/// The `toolu delivery-flow` namespace.
pub fn command() -> Command {
  GUIDE.command()
}

/// `toolu delivery-flow`: print the guide.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  GUIDE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
