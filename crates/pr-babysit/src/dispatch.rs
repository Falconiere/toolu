//! `dispatch-fix start|wait|cleanup`: run a route plan's groups one after
//! another, each a fixer agent in a herdr pane, and settle each from its report.

use serde_json::Value;

use crate::error::{Failure, Result};

pub(crate) mod brief;
pub(crate) mod cleanup;
pub(crate) mod dispatcher;
pub(crate) mod herdr;
pub(crate) mod launch;
pub(crate) mod names;
pub(crate) mod outcome;
pub(crate) mod plan;
#[cfg(test)]
#[path = "tests/sandbox_test.rs"]
mod sandbox;
pub(crate) mod start;
pub(crate) mod watch;
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
