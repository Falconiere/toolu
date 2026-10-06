//! The ts-quality rule crate: its rules and the `toolu ts-quality` namespace, not ported
//! yet (#426). `crates/cli` reaches it through the hub (`crates/toolu`).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/ts-quality`.
pub const PLUGIN: &str = "ts-quality";

const NAMESPACE: Planned = Planned {
  name: "ts-quality",
  about: "TypeScript quality rules that run after each edit",
  verbs: &[],
  issues: &[426],
};

/// The `toolu ts-quality` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu ts-quality` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
