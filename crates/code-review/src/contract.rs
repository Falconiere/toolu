//! The report `toolu code-review run` prints: the machine-readable contract agents
//! read instead of prose (epic #660). Its JSON schema is `$defs/codeReview` in
//! `crates/cli/src/commands.schema.json`. The verdict is computed from validated
//! findings, never taken from the model; `blocker`, `high` and `medium` block.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize, Serializer};
use toolu_protocol::exit::Exit;

use crate::budget::Effort;

/// The contract's version; a breaking change to the report raises it.
pub const SCHEMA_VERSION: u32 = 3;

/// How bad a finding is; ordered from `nit` up, so `>= Medium` means blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
  /// Advisory style or wording.
  Nit,
  /// Advisory.
  Low,
  /// A defect worth fixing before merge.
  Medium,
  /// A real defect.
  High,
  /// Must not ship.
  Blocker,
}

impl Severity {
  /// Whether the finding blocks the push: `blocker`, `high` and `medium`.
  pub fn blocking(self) -> bool {
    self >= Severity::Medium
  }

  /// The name the report uses.
  pub fn name(self) -> &'static str {
    match self {
      Severity::Nit => "nit",
      Severity::Low => "low",
      Severity::Medium => "medium",
      Severity::High => "high",
      Severity::Blocker => "blocker",
    }
  }
}

/// Which side of the diff a finding's lines number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
  /// The new file: added or context lines.
  New,
  /// The old file: removed lines, including replacements and deleted files.
  Old,
}

/// What produced a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
  /// The review model.
  Llm,
  /// An ast-grep rule of the pre-pass.
  AstGrep,
  /// The secret scanner of the pre-pass.
  Secrets,
  /// A local failure that prevented model judgment.
  System,
}

impl Source {
  /// The name the report uses.
  pub fn name(self) -> &'static str {
    match self {
      Source::Llm => "llm",
      Source::AstGrep => "ast-grep",
      Source::Secrets => "secrets",
      Source::System => "system",
    }
  }
}

/// A byte range of the unmasked source line that held a secret: `start` inclusive,
/// `end` exclusive, UTF-8 offsets. Later stages mask outbound text with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RedactionSpan {
  /// First byte of the secret.
  pub start: usize,
  /// One past the last byte of the secret.
  pub end: usize,
}

/// One anchored finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub suggestion: Option<String>,
  /// Byte ranges of secrets in the unmasked line; the `quote` is already masked.
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub redaction_spans: Vec<RedactionSpan>,
}

/// The run's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
  /// No blocking finding and full coverage.
  Approved,
  /// At least one blocking finding, or a path left unreviewed.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Omitted {
  /// Repository-relative path.
  pub path: String,
  /// Why it was not reviewed.
  pub reason: String,
}

/// What the run covered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Coverage {
  /// Paths reviewed in full.
  pub reviewed: Vec<String>,
  /// Paths that should have been reviewed and were not; never a pass.
  pub unreviewed: Vec<Omitted>,
  /// Noise-policy skips: lockfiles, vendored, generated and binary files.
  pub skipped: Vec<Omitted>,
}

/// What the run cost.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Usage {
  /// Prompt tokens.
  pub tokens_in: u64,
  /// Completion tokens, reasoning included.
  pub tokens_out: u64,
  /// Reported cost, or unknown when the provider omitted it.
  pub cost_usd: Option<Cost>,
  /// The model that served the review, when one did.
  pub model: Option<String>,
  /// Metadata for each external call, with no prompt or source text.
  pub calls: Vec<crate::review::Call>,
}

/// A cost in US dollars: finite and non-negative by construction, so a report
/// never turns an invalid number into a reported cost.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Cost(f64);

impl Cost {
  /// `usd` as a cost, or `None` when it is negative, NaN or infinite.
  pub fn usd(usd: f64) -> Option<Cost> {
    (usd.is_finite() && usd >= 0.0).then_some(Cost(usd))
  }
}

/// Why a run could not review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
  /// Missing or unusable key, or a pipeline not built: nothing was sent.
  Unconfigured,
  /// The repository has not opted in: nothing was sent.
  NotEnabled,
  /// Every model call failed.
  Outage,
  /// Every review call ran out of its level's wall deadline.
  Deadline,
  /// The review finished, but local state could not be saved.
  State,
}

impl ErrorKind {
  /// The name the report uses.
  pub fn name(self) -> &'static str {
    match self {
      ErrorKind::Unconfigured => "unconfigured",
      ErrorKind::NotEnabled => "not-enabled",
      ErrorKind::Outage => "outage",
      ErrorKind::Deadline => "deadline",
      ErrorKind::State => "state",
    }
  }
}

/// The error of a run whose verdict is `error`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportError {
  /// Which failure.
  pub kind: ErrorKind,
  /// An actionable line for a human.
  pub message: String,
}

/// The whole report. Its verdict and finding order are computed whenever they are
/// read or printed, never stored, so editing the findings or coverage cannot leave
/// them stale, and nothing can take a verdict from the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
  /// The effort level of the run.
  pub effort: Effort,
  /// Findings, in any order; the report prints them worst first.
  pub findings: Vec<Finding>,
  /// What was and was not reviewed.
  pub coverage: Coverage,
  /// What it cost.
  pub usage: Usage,
  /// Why the run could not review; when set, the verdict is `error`.
  pub error: Option<ReportError>,
  /// Where the effort level came from and any deterministic floor.
  pub effort_source: Option<String>,
  /// Resolved, still-open and new finding IDs.
  pub recap: Vec<String>,
  /// Pre-pass findings and notices; retained during an outage.
  pub mechanical_checks: Vec<String>,
  /// Review degradation notes, not full source evidence.
  pub notes: Vec<String>,
}

/// The printed document: `Report` plus its versions and computed verdict.
#[derive(Serialize)]
struct Document<'a> {
  schema_version: u32,
  toolu_version: &'static str,
  verdict: Verdict,
  effort: Effort,
  findings: Vec<&'a Finding>,
  coverage: &'a Coverage,
  usage: &'a Usage,
  error: Option<&'a ReportError>,
  #[serde(skip_serializing_if = "Option::is_none")]
  effort_source: Option<&'a String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  recap: &'a Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  mechanical_checks: &'a Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  notes: &'a Vec<String>,
}

impl Serialize for Report {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let mut findings: Vec<&Finding> = self.findings.iter().collect();
    findings.sort_by_key(|finding| Reverse(finding.severity));
    Document {
      schema_version: SCHEMA_VERSION,
      toolu_version: env!("CARGO_PKG_VERSION"),
      verdict: self.verdict(),
      effort: self.effort,
      findings,
      coverage: &self.coverage,
      usage: &self.usage,
      error: self.error.as_ref(),
      effort_source: self.effort_source.as_ref(),
      recap: &self.recap,
      mechanical_checks: &self.mechanical_checks,
      notes: &self.notes,
    }
    .serialize(serializer)
  }
}

impl Report {
  /// A reviewed run.
  pub fn reviewed(
    effort: Effort,
    findings: Vec<Finding>,
    coverage: Coverage,
    usage: Usage,
  ) -> Report {
    Report {
      effort,
      findings,
      coverage,
      usage,
      error: None,
      effort_source: None,
      recap: Vec::new(),
      mechanical_checks: Vec::new(),
      notes: Vec::new(),
    }
  }

  /// A run that could not review: verdict `error`, nothing covered.
  pub fn failed(effort: Effort, kind: ErrorKind, message: &str) -> Report {
    Report {
      error: Some(ReportError {
        kind,
        message: message.to_owned(),
      }),
      ..Report::reviewed(effort, Vec::new(), Coverage::default(), Usage::default())
    }
  }

  /// The verdict, computed and never taken from the model: `error` when the run could
  /// not review, `blocked` when a finding blocks or any path went unreviewed, else
  /// `approved`.
  pub fn verdict(&self) -> Verdict {
    if self.error.is_some() {
      Verdict::Error
    } else if self.blocking_count() > 0 || !self.coverage.unreviewed.is_empty() {
      Verdict::Blocked
    } else {
      Verdict::Approved
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

  /// Whether the run reviewed the whole clean committed diff: no error and no
  /// unreviewed path. Only a complete review may leave a passing gate state.
  pub fn is_complete(&self) -> bool {
    self.error.is_none() && self.coverage.unreviewed.is_empty()
  }

  /// The process exit: 0 approved, 2 blocked, 69 when the run could not review.
  pub fn exit(&self) -> Exit {
    match self.verdict() {
      Verdict::Approved => Exit::Success,
      Verdict::Blocked => Exit::Blocked,
      Verdict::Error => Exit::Unavailable,
    }
  }
}

#[cfg(test)]
#[path = "tests/contract_test.rs"]
mod tests;
