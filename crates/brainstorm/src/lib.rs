//! The brainstorm plugin's crate: `toolu brainstorm`, the guide of a Markdown-only plugin.

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The plugin this crate belongs to: `plugins/brainstorm`.
pub const PLUGIN: &str = "brainstorm";

const GUIDE: Guide = Guide {
  name: "brainstorm",
  about: "Think a change through before building (a Markdown-only plugin)",
  text: "brainstorm is a Markdown-only plugin: it has no verbs. Its skill thinks a change through \
   before building: evidence-backed triage, alternatives, trade-offs and a recommended \
   approach, without editing code.\n\nRun it as /brainstorm:brainstorm in Claude Code or \
   $brainstorm:brainstorm in Codex. delivery-flow runs it as its first phase.",
};

/// The `toolu brainstorm` namespace.
pub fn command() -> Command {
  GUIDE.command()
}

/// `toolu brainstorm`: print the guide.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  GUIDE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
