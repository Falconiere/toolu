//! The document `toolu code-review eval` prints and writes: one `EvalReport` per
//! case, what could not be verified, the offline gate-contract violations, and the
//! calibration over every case when more than one effort setting ran.

use serde::Serialize;

use super::calibration::Calibration;
use super::expectations::Class;
use super::report::EvalReport;
use super::setting::Setting;

/// The suite document's version.
pub const SCHEMA: u32 = 1;

/// One case's paired comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseReport {
  /// The case.
  pub id: String,
  /// Its fixture class.
  pub class: Class,
  /// The pinned diff and every arm that ran.
  #[serde(flatten)]
  pub report: EvalReport,
  /// Arms that could not be verified: a replay miss, a provider outage, a store
  /// that could not be written. Never a pass.
  pub unverified: Vec<String>,
  /// Gate-contract flags the case asserts that the production predicates refuse.
  pub gate_violations: Vec<String>,
}

/// The whole document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SuiteReport {
  /// Always [`SCHEMA`].
  pub schema: u32,
  /// Every case that ran.
  pub cases: Vec<CaseReport>,
  /// Per-setting totals and `auto`'s choices, when several settings ran.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub calibration: Option<Calibration>,
}

/// How a run ended, in the order exit codes take precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
  /// Every required expectation held.
  Satisfied,
  /// A defect was missed or a gate-contract flag failed.
  Missed,
  /// Something could not be verified: a replay miss, an unavailable provider, an
  /// incomplete review, or a case in which no arm ran.
  Unverified,
}

impl SuiteReport {
  /// How the run ended. A `none` arm is the known-weak baseline calibration measures
  /// against, so its misses never fail the run; every other arm must catch every
  /// defect row. A case with no arm, or only `none` arms, proved nothing, so it is
  /// never satisfied.
  pub fn verdict(&self) -> Verdict {
    if self
      .cases
      .iter()
      .any(|case| !case.unverified.is_empty() || proved_nothing(case))
    {
      return Verdict::Unverified;
    }
    let missed = self.cases.iter().any(|case| {
      !case.gate_violations.is_empty()
        || case
          .report
          .arms
          .iter()
          .any(|arm| arm.setting != Setting::None && arm.tally.missed > 0)
    });
    if missed {
      Verdict::Missed
    } else {
      Verdict::Satisfied
    }
  }

  /// One line per arm of every case, then what failed and the calibration summary.
  pub fn summary(&self) -> String {
    let mut lines = Vec::new();
    for case in &self.cases {
      lines.extend(case.report.arms.iter().map(|arm| {
        let tally = arm.tally;
        format!(
          "{} {:?}/{} caught {} missed {} flagged {} unclassified {} · {} tokens · ${:.4}",
          case.id,
          arm.arm,
          arm.setting.name(),
          tally.caught,
          tally.missed,
          tally.flagged,
          tally.unclassified,
          arm.usage.tokens_in.saturating_add(arm.usage.tokens_out),
          arm.usage.cost_usd
        )
      }));
      if proved_nothing(case) && case.unverified.is_empty() {
        lines.push(format!(
          "{}: unverified: no arm ran beyond the known-weak none baseline",
          case.id
        ));
      }
      lines.extend(
        case
          .unverified
          .iter()
          .map(|why| format!("{}: unverified: {why}", case.id)),
      );
      lines.extend(
        case
          .gate_violations
          .iter()
          .map(|why| format!("{}: gate contract: {why}", case.id)),
      );
    }
    if let Some(calibration) = &self.calibration {
      lines.push(format!(
        "calibration: auto chose below the lowest catching level in {} of {} runs",
        calibration.auto_below_lowest,
        calibration.auto.len()
      ));
    }
    lines.join("\n")
  }
}

/// Whether `case` ran no arm whose misses count: none at all, or only `none`.
fn proved_nothing(case: &CaseReport) -> bool {
  case
    .report
    .arms
    .iter()
    .all(|arm| arm.setting == Setting::None)
}

#[cfg(test)]
#[path = "tests/suite_test.rs"]
mod tests;
