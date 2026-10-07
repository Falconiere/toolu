//! The statusline plugin's crate: the `toolu statusline` namespace, not ported yet (#431).

use clap::{ArgMatches, Command};
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/statusline`.
pub const PLUGIN: &str = "statusline";

const NAMESPACE: Planned = Planned {
  name: "statusline",
  about: "Project status for the host's status line",
  verbs: &["render", "setup", "refresh"],
  issues: &[431],
};

/// The `toolu statusline` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu statusline` verb with the status snapshot `crates/cli` passes
/// in (the hub's): until the port renders it (#431), the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx, _status: &dyn StatusSnapshot) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
