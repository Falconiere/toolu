//! The epic-orchestrator plugin's crate: the `toolu epic` namespace, not ported yet
//! (#434, #435, #448).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

/// The plugin this crate belongs to: `plugins/epic-orchestrator`.
pub const PLUGIN: &str = "epic-orchestrator";

const NAMESPACE: Planned = Planned {
  name: "epic",
  about: "Drive an epic to merged PRs: the resident engine, merge queue and trackers",
  verbs: &[
    "engine", "start", "status", "pause", "resume", "ack", "answer", "wait", "report", "job",
    "graph", "route", "launch", "finish", "close", "release", "jira", "probe", "gate", "queue",
  ],
  issues: &[434, 435, 448],
};

/// The `toolu epic` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu epic` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
