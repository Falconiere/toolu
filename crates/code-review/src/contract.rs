//! The report `toolu code-review run` prints: the machine-readable contract agents
//! read instead of prose (epic #660). Its JSON schema is `$defs/codeReview` in
//! `crates/cli/src/commands.schema.json`. The verdict is computed from validated
//! findings, never taken from the model; `blocker`, `high` and `medium` block.

use serde::{Deserialize, Serialize};
use toolu_protocol::exit::Exit;

use crate::budget::Effort;

/// The contract's version; a breaking change to the report raises it.
pub const SCHEMA_VERSION: u32 = 1;

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
  /// Must not ship.
  Blocker,
  /// A real defect.
  High,
  /// A defect worth fixing before merge.
  Medium,
  /// Advisory.
  Low,
  /// Advisory style or wording.
  Nit,
}

impl Severity {
  /// Whether the finding blocks the push: `blocker`, `high` and `medium`.
  pub fn blocking(self) -> bool {
    matches!(self, Severity::Blocker | Severity::High | Severity::Medium)
  }
}

/// Which side of the diff a finding's lines number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
  /// The new file: added or context lines.
  New,
  /// The old file: removal-only hunks and deleted files.
  Old,
}

/// What produced a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
  /// The review model.
  Llm,
  /// An ast-grep rule of the pre-pass.
  AstGrep,
  /// The secret scanner of the pre-pass.
  Secrets,
}

/// One anchored finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
  /// Stable within a branch's review memory.
  pub id: String,
  /// Repository-relative path.
  pub path: String,
  /// First line, on `side`.
  pub line: u32,
  /// Last line, on `side`.
  pub end_line: u32,
  /// The side `line` and `end_line` number.
  pub side: Side,
  /// How bad it is.
  pub severity: Severity,
  /// Its kind, for example `correctness`; part of the memory fingerprint.
  pub category: String,
  /// What produced it.
  pub source: Source,
  /// The defect, in one sentence.
  pub claim: String,
  /// The diff text it is grounded in.
  pub quote: String,
  /// A fix, when there is one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub suggestion: Option<String>,
}

/// The run's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
  /// No blocking finding and full coverage.
  Approved,
  /// At least one blocking finding.
  Blocked,
  /// The run could not review; `error` says why.
  Error,
}

impl Verdict {
  /// The name the report uses.
  pub fn name(self) -> &'static str {
    match self {
      Verdict::Approved => "approved",
      Verdict::Blocked => "blocked",
      Verdict::Error => "error",
    }
  }
}

/// A path left out of the review, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Omitted {
  /// Repository-relative path.
  pub path: String,
  /// Why it was not reviewed.
  pub reason: String,
}

/// What the run covered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
  /// Paths reviewed in full.
  pub reviewed: Vec<String>,
  /// Paths that should have been reviewed and were not; never a pass.
  pub unreviewed: Vec<Omitted>,
  /// Noise-policy skips: lockfiles, vendored, generated and binary files.
  pub skipped: Vec<Omitted>,
}

/// What the run cost.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
  /// Prompt tokens.
  pub tokens_in: u64,
  /// Completion tokens, reasoning included.
  pub tokens_out: u64,
  /// Reported cost in US dollars.
  pub cost_usd: f64,
  /// The model that served the review, when one did.
  pub model: Option<String>,
}

/// Why a run could not review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorKind {
  /// Missing key, missing opt-in, or a pipeline not built: nothing was sent.
  Unconfigured,
  /// Every model call failed.
  Outage,
}

impl ErrorKind {
  /// The name the report uses.
  pub fn name(self) -> &'static str {
    match self {
      ErrorKind::Unconfigured => "unconfigured",
      ErrorKind::Outage => "outage",
    }
  }
}

/// The error of a run whose verdict is `error`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportError {
  /// Which failure.
  pub kind: ErrorKind,
  /// An actionable line for a human.
  pub message: String,
}

/// The whole report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
  /// `SCHEMA_VERSION`.
  pub schema_version: u32,
  /// The `toolu` build that produced it.
  pub toolu_version: String,
  /// The computed verdict.
  pub verdict: Verdict,
  /// The effort level of the run.
  pub effort: Effort,
  /// Findings, worst first.
  pub findings: Vec<Finding>,
  /// What was and was not reviewed.
  pub coverage: Coverage,
  /// What it cost.
  pub usage: Usage,
  /// Set exactly when `verdict` is `error`.
  pub error: Option<ReportError>,
}

impl Report {
  /// A run that could not review: verdict `error`, nothing covered.
  pub fn failed(effort: Effort, kind: ErrorKind, message: &str) -> Report {
    Report {
      schema_version: SCHEMA_VERSION,
      toolu_version: env!("CARGO_PKG_VERSION").to_owned(),
      verdict: Verdict::Error,
      effort,
      findings: Vec::new(),
      coverage: Coverage::default(),
      usage: Usage::default(),
      error: Some(ReportError {
        kind,
        message: message.to_owned(),
      }),
    }
  }

  /// The findings that block: the gate's `findings_count`.
  pub fn blocking_count(&self) -> usize {
    self
      .findings
      .iter()
      .filter(|finding| finding.severity.blocking())
      .count()
  }

  /// The process exit: 0 approved, 2 blocked, 69 when the run could not review.
  pub fn exit(&self) -> Exit {
    match self.verdict {
      Verdict::Approved => Exit::Success,
      Verdict::Blocked => Exit::Blocked,
      Verdict::Error => Exit::Unavailable,
    }
  }
}

#[cfg(test)]
#[path = "tests/contract_test.rs"]
mod tests;
