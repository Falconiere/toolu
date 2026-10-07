//! Cross-plugin links (#460): plugin logic one plugin owns and another uses
//! crosses as a `toolu-engine` trait. The owner implements it, and this module
//! passes that implementation to the user when the registry names its
//! namespace, so no plugin crate depends on another.

use clap::ArgMatches;
use toolu_engine::babysit::BabysitTick;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::{Ctx, Outcome};

/// pr-babysit's tick, which the epic engine runs.
pub(crate) const BABYSIT_TICK: &dyn BabysitTick = &toolu_pr_babysit::Tick;

/// The hub's status snapshot, which statusline renders.
pub(crate) const STATUS_SNAPSHOT: &dyn StatusSnapshot = &toolu_hub::status::Snapshot;

/// `toolu epic`, with pr-babysit's tick.
pub(crate) fn epic(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  toolu_epic_orchestrator::run(matches, ctx, BABYSIT_TICK)
}

/// `toolu statusline`, with the hub's status snapshot.
pub(crate) fn statusline(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  toolu_statusline::run(matches, ctx, STATUS_SNAPSHOT)
}

#[cfg(test)]
#[path = "tests/links_test.rs"]
mod tests;
