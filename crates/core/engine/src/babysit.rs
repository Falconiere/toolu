//! `BabysitTick`: one babysit check of a pull request. pr-babysit owns it, the
//! epic engine runs it, and `crates/cli` hands the engine pr-babysit's
//! implementation, so neither plugin crate depends on the other.

use std::path::PathBuf;

use serde_json::Value;

use crate::LinkError;

/// The pull request a tick checks and the state file it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickRequest {
  /// `owner/repo`.
  pub repo: String,
  /// The pull request number.
  pub number: u64,
  /// The tick's state file, which carries one check's findings to the next.
  pub state_file: PathBuf,
  /// The check's time as `YYYY-MM-DDTHH:MM:SSZ`; the current time when `None`.
  pub now: Option<String>,
}

/// What a tick concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickDecision {
  /// Something is still running or there is something to fix.
  KeepGoing,
  /// CI is green and no thread is unresolved, and the review verdict is clean
  /// or could not be read (pr-babysit's degraded success; verify it by hand).
  Success,
  /// A human or the worker has to look.
  Escalate,
}

/// A tick's decision and its full result document, kept for the journal.
#[derive(Debug, Clone, PartialEq)]
pub struct TickReport {
  /// The decision.
  pub decision: TickDecision,
  /// The tick's result as `toolu babysit tick --json` prints it.
  pub result: Value,
}

/// One babysit check of a pull request.
pub trait BabysitTick: Send + Sync {
  /// Check `request`'s pull request once and update its state file.
  ///
  /// # Errors
  /// [`LinkError`] when the tick could not run, which the engine turns into an
  /// attention item.
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError>;
}

#[cfg(test)]
#[path = "tests/babysit_test.rs"]
mod tests;
