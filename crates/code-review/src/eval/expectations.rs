//! The committed expectations file: the cases a harness run is judged against.
//! The matcher reads `Expectation` rows; the runner (a later step) reads the rest.

use serde::{Deserialize, Serialize};

/// The expectations file's version; any other value is refused.
pub const SET_VERSION: u32 = 1;

/// What an expectation row asserts about a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
  /// A real defect the review must report.
  Defect,
  /// A known false positive; reporting it is counted against the review.
  FalsePositive,
}

/// The fixture class of a case, ported from the upstream review fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
  /// The defect is evidenced in the diff.
  Supported,
  /// The evidence contradicts the claim.
  Contradicted,
  /// The evidence neither shows nor refutes the claim.
  Insufficient,
  /// The diff tries to steer the reviewer.
  Adversarial,
  /// The defect spans more than one file.
  CrossFile,
  /// A prompt-induced false positive.
  PromptFalsePositive,
  /// A recorded line moved; the finding stays open.
  MemoryDrift,
  /// A prior finding nobody re-examined; it stays open.
  MemoryUnreexamined,
  /// No review history; the review runs in full.
  MemoryMissing,
}

/// One row: a path and a distinctive claim substring, and what finding it should be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expectation {
  /// Stable within its case.
  pub id: String,
  /// Repository-relative path a finding must carry exactly.
  pub path: String,
  /// A substring of the finding's claim; case is ignored.
  pub claim: String,
  /// Whether a match is a caught defect or a flagged false positive.
  pub kind: Kind,
}

/// The gate-contract flags a case asserts offline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Gate {
  /// Blocking severities drive the blocking count.
  pub blocking_severities_drive_count: bool,
  /// Advisory findings never block.
  pub advisory_never_blocks: bool,
  /// An unreviewed path never writes a passing state.
  pub unreviewed_never_writes_pass: bool,
}

/// One pinned diff and what a review of it must and must not report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
  /// Names the case and its fixture directory.
  pub id: String,
  /// Its fixture class.
  pub class: Class,
  /// The base revision of the diff, when not the tip's parent.
  #[serde(default)]
  pub base: Option<String>,
  /// A prior review-memory seed for the memory classes.
  #[serde(default)]
  pub memory: Option<serde_json::Value>,
  /// What the review is judged against.
  pub expectations: Vec<Expectation>,
  /// The gate-contract flags, when the case asserts them.
  #[serde(default)]
  pub gate: Option<Gate>,
}

/// The whole expectations file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseSet {
  /// Must equal [`SET_VERSION`].
  pub version: u32,
  /// Every case.
  pub cases: Vec<Case>,
}

impl CaseSet {
  /// Parse the expectations file's text.
  ///
  /// # Errors
  /// An unreadable document or an unknown version, never an empty set.
  pub fn parse(text: &str) -> Result<CaseSet, String> {
    let set: CaseSet =
      serde_json::from_str(text).map_err(|error| format!("invalid expectations: {error}"))?;
    if set.version == SET_VERSION {
      Ok(set)
    } else {
      Err(format!(
        "unsupported expectations version {} (expected {SET_VERSION})",
        set.version
      ))
    }
  }

  /// The case named `id`.
  pub fn case(&self, id: &str) -> Option<&Case> {
    self.cases.iter().find(|case| case.id == id)
  }
}

#[cfg(test)]
#[path = "tests/expectations_test.rs"]
mod tests;
