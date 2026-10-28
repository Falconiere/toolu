//! Pair findings with expectation rows. A finding matches a row when its path equals
//! the row's path and the row's claim is a case-insensitive substring of the finding's
//! claim; the first unused matching finding wins. Findings no row claimed are
//! `unclassified` for a human to adjudicate.

use serde::Serialize;

use super::expectations::{Expectation, Kind};
use crate::contract::Finding;

/// How one expectation row came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
  /// A defect row a finding matched.
  Caught,
  /// A defect row no finding matched: a miss.
  Missed,
  /// A false-positive row a finding matched: the known false positive was raised.
  Flagged,
  /// A false-positive row no finding matched: the review stayed clear of it.
  Clear,
}

/// One row's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Match {
  /// The expectation's id.
  pub expectation_id: String,
  /// How it came out.
  pub status: Status,
  /// The finding that matched, when one did.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub finding_id: Option<String>,
}

/// Counts of each status, for the quality summary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
  /// Defect rows matched.
  pub caught: usize,
  /// Defect rows unmatched.
  pub missed: usize,
  /// False-positive rows matched.
  pub flagged: usize,
  /// False-positive rows unmatched.
  pub clear: usize,
  /// Findings no row claimed.
  pub unclassified: usize,
}

/// Every row's outcome and the findings left over.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
  /// One entry per expectation, in the given order.
  pub matches: Vec<Match>,
  /// Findings no expectation claimed.
  pub unclassified: Vec<Finding>,
}

impl Outcome {
  /// Count the statuses.
  pub fn tally(&self) -> Tally {
    let count = |status: Status| {
      self
        .matches
        .iter()
        .filter(|entry| entry.status == status)
        .count()
    };
    Tally {
      caught: count(Status::Caught),
      missed: count(Status::Missed),
      flagged: count(Status::Flagged),
      clear: count(Status::Clear),
      unclassified: self.unclassified.len(),
    }
  }

  /// Whether every defect row was caught; known false positives and unclassified
  /// findings do not fail a case on their own.
  pub fn defects_satisfied(&self) -> bool {
    self.tally().missed == 0
  }
}

/// Whether `finding` satisfies `expectation`'s path and claim.
fn fits(expectation: &Expectation, finding: &Finding) -> bool {
  finding.path == expectation.path
    && finding
      .claim
      .to_lowercase()
      .contains(&expectation.claim.to_lowercase())
}

fn status(kind: Kind, matched: bool) -> Status {
  match (kind, matched) {
    (Kind::Defect, true) => Status::Caught,
    (Kind::Defect, false) => Status::Missed,
    (Kind::FalsePositive, true) => Status::Flagged,
    (Kind::FalsePositive, false) => Status::Clear,
  }
}

/// Match `findings` against `expectations`. Each finding satisfies at most one row.
pub fn match_findings(expectations: &[Expectation], findings: &[Finding]) -> Outcome {
  let mut used = vec![false; findings.len()];
  let matches = expectations
    .iter()
    .map(|expectation| {
      let hit = findings.iter().enumerate().find(|(index, finding)| {
        !used.get(*index).copied().unwrap_or(true) && fits(expectation, finding)
      });
      if let Some((index, _)) = hit
        && let Some(slot) = used.get_mut(index)
      {
        *slot = true;
      }
      Match {
        expectation_id: expectation.id.clone(),
        status: status(expectation.kind, hit.is_some()),
        finding_id: hit.map(|(_, finding)| finding.id.clone()),
      }
    })
    .collect();
  let unclassified = findings
    .iter()
    .zip(&used)
    .filter(|(_, taken)| !**taken)
    .map(|(finding, _)| finding.clone())
    .collect();
  Outcome {
    matches,
    unclassified,
  }
}

#[cfg(test)]
#[path = "tests/matcher_test.rs"]
mod tests;
