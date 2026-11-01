//! The epic-creator plugin's crate: `toolu epic-creator`, the guide of a Markdown-only plugin.

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Guide;

/// The plugin this crate belongs to: `plugins/epic-creator`.
pub const PLUGIN: &str = "epic-creator";

const GUIDE: Guide = Guide {
  name: "epic-creator",
  about: "Turn an idea into a GitHub epic the epic engine can run (a Markdown-only plugin)",
  text: "epic-creator is a Markdown-only plugin: it has no verbs. Its skill thinks an idea \
   through, splits it into one-PR sub-issues with cited decisions and runnable acceptance \
   checks, and after one confirmation creates the GitHub epic, its native sub-issues and \
   their dependencies.\n\nRun it as /epic-creator:epic-creator in Claude Code or \
   $epic-creator:epic-creator in Codex, then run the epic with epic-orchestrator.",
};

/// The `toolu epic-creator` namespace.
pub fn command() -> Command {
  GUIDE.command()
}

/// `toolu epic-creator`: print the guide.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  GUIDE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
