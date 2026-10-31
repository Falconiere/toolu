//! `BabysitTick`: one babysit check of a pull request. pr-babysit owns it, the
//! epic engine runs it, and `crates/cli` hands the engine pr-babysit's
//! implementation, so neither plugin crate depends on the other.

use std::hash::{DefaultHasher, Hash, Hasher};
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
  /// A human has to look; the report's `prompt`, when set, also goes to the
  /// worker. Reported once, when the escalation is new.
  Escalate,
  /// New work for the worker: the report's `prompt` says what.
  Act,
}

/// A tick's decision and its full result document, kept for the journal.
#[derive(Debug, Clone, PartialEq)]
pub struct TickReport {
  /// The decision.
  pub decision: TickDecision,
  /// The tick's result as `toolu babysit tick --json` prints it.
  pub result: Value,
  /// GraphQL points and primary limit observed by this tick, when available.
  pub graphql: Option<GraphQlUsage>,
  /// The text to send the worker, with [`TickDecision::Act`] or
  /// [`TickDecision::Escalate`].
  pub prompt: Option<String>,
  /// [`state_digest`] of the state file bytes this tick wrote, when it wrote
  /// them, so a later read can tell whether anyone has written since.
  pub state_digest: Option<u64>,
}

/// The digest of a babysit state file's bytes that a tick reports and the
/// engine compares a later read against; stable within one process.
pub fn state_digest(bytes: &[u8]) -> u64 {
  let mut hasher = DefaultHasher::new();
  bytes.hash(&mut hasher);
  hasher.finish()
}

/// The GraphQL budget observed by one babysit tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphQlUsage {
  /// Points reported by `rateLimit.cost` across the tick's GraphQL calls.
  pub points: u64,
  /// Latest `x-ratelimit-remaining` value, when GitHub sent it.
  pub remaining: Option<u64>,
  /// `x-ratelimit-reset`, in Unix seconds, when GitHub sent it.
  pub reset_at: Option<u64>,
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

impl BabysitTick for () {
  fn tick(&self, _request: &TickRequest) -> Result<TickReport, LinkError> {
    Err(LinkError::NotPorted { issue: 433 })
  }
}

#[cfg(test)]
#[path = "tests/babysit_test.rs"]
mod tests;
