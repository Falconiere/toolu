//! The toolu-review plugin: native `toolu review write-state` and `SessionStart` hooks.

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

mod check;
mod cli;
mod git;
mod session;
mod state;
mod writer;

pub use check::check_binary;
pub use session::session_start;

/// The plugin this crate belongs to: `plugins/toolu-review`.
pub const PLUGIN: &str = "toolu-review";

/// The `toolu review` namespace.
pub fn command() -> Command {
  cli::command()
}

/// Run the native review-state writer.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  writer::run(matches, ctx, &Env::process())
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
