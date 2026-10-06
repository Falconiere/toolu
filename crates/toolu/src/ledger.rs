//! `toolu ledger`: the plan ledger, verdict and push-waiver verbs, not ported yet (#421).

use clap::{ArgMatches, Command};
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::namespace::Planned;

const NAMESPACE: Planned = Planned {
  name: "ledger",
  about: "The plan ledger, verdicts and push waivers",
  verbs: &[
    "run",
    "status",
    "preflight",
    "path",
    "root",
    "self-test",
    "verdict",
  ],
  issues: &[421],
};

/// The `toolu ledger` namespace.
pub fn command() -> Command {
  NAMESPACE.command()
}

/// Run a `toolu ledger` verb: until its port, the placeholder.
pub fn run(_matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  NAMESPACE.run(ctx)
}

#[cfg(test)]
#[path = "tests/ledger_test.rs"]
mod tests;
