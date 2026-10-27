//! The review-memory records carried between rounds in the per-branch code-review
//! state file, chained by `diff_sha` (epic #660 review memory). #666 and #668 read
//! them and #669 persists them; the push-review gate file never holds them.

use serde::{Deserialize, Serialize};

use crate::contract::{Severity, Verdict};

/// Where a prior finding stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
  /// Still open: not fixed, or not re-examined this round.
  Open,
  /// Fixed in a later diff.
  Resolved,
  /// Settled by an explicit dismissal.
  Dismissed,
}

/// A finding from an earlier round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorFinding {
  /// Line-independent: `path + category + normalized text` (normalized by #668).
  pub fingerprint: String,
  /// The finding's id.
  pub id: String,
  /// Its severity.
  pub severity: Severity,
  /// Where it stands.
  pub status: Status,
}

/// An agent's dismissal of a finding; a blocker matches by exact id only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dismissal {
  /// The dismissed finding's id.
  pub id: String,
  /// Why it does not apply.
  pub reason: String,
  /// Who dismissed it.
  pub by: String,
}

/// Blocking and advisory finding counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
  /// `blocker`, `high` and `medium`.
  pub blocking: usize,
  /// `low` and `nit`.
  pub advisory: usize,
}

/// One past review pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pass {
  /// The reviewed diff's hash, as the push-review gate computes it.
  pub diff_sha: String,
  /// The pass's verdict.
  pub verdict: Verdict,
  /// Its finding counts.
  pub counts: Counts,
}
