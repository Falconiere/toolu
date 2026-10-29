//! `dispatch-fix start|wait|cleanup`: run a route plan's groups one after
//! another, each a fixer agent in a herdr pane, and settle each from its report.

use serde_json::Value;

use crate::error::{Failure, Result};

/// Render fixer briefs.
pub(crate) mod brief;
/// Retire completed fixer work.
pub(crate) mod cleanup;
/// Persist and execute fixer dispatch operations.
pub(crate) mod dispatcher;
/// Manage owned herdr-core fixer terminals.
pub(crate) mod hc;
/// Call the legacy Herdr backend.
pub(crate) mod herdr;
/// Prepare and launch a fixer group.
pub(crate) mod launch;
/// Name fixer agents and round artifacts.
pub(crate) mod names;
/// Classify fixer reports and screens.
pub(crate) mod outcome;
/// Validate fixer route plans.
pub(crate) mod plan;
/// Transfer bound remote fixer results.
pub(crate) mod remote;
/// Verify and retire remote worktrees.
pub(crate) mod remote_finish;
/// Rotate completed remote attempts safely.
pub(crate) mod remote_round;
/// Prepare committed remote fixer inputs.
pub(crate) mod remote_setup;
#[cfg(test)]
#[path = "tests/sandbox_test.rs"]
mod sandbox;
/// Begin a fixer round.
pub(crate) mod start;
/// Follow fixer execution and settlement.
pub(crate) mod watch;
/// Prepare and reuse slot worktrees.
pub(crate) mod worktree;

/// A `dispatch-fix` subcommand.
pub(crate) enum Verb<'a> {
  /// Start a round.
  Start(start::Start<'a>),
  /// Follow it, for the given `--timeout-seconds` if any.
  Wait(Option<&'a str>),
  /// End it and remove what it left.
  Cleanup,
}

/// Runs `verb` over the slot `options` names; its JSON document.
///
/// # Errors
/// `usage` for a malformed timeout or a dry wait; the subcommand's failures.
pub(crate) fn dispatch_fix(verb: &Verb<'_>, options: &dispatcher::Options<'_>) -> Result<Value> {
  if options.dry && matches!(verb, Verb::Wait(_)) {
    return Err(Failure::usage(
      "--dry-run applies to start and cleanup, not wait",
    ));
  }
  let timeout = match verb {
    Verb::Wait(Some(text)) => text
      .parse::<u64>()
      .map_err(|_error| Failure::usage("--timeout-seconds must be a whole number"))?,
    Verb::Start(_) | Verb::Wait(None) | Verb::Cleanup => watch::DEFAULT_TIMEOUT,
  };
  let mut dispatcher = dispatcher::Dispatcher::new(options)?;
  match verb {
    Verb::Start(start) => dispatcher.start(start),
    Verb::Wait(_) => dispatcher.wait(timeout),
    Verb::Cleanup => dispatcher.cleanup(),
  }
}

#[cfg(test)]
#[path = "tests/dispatch_test.rs"]
mod tests;
