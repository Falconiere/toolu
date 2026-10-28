//! Effort calibration over a set of arm reports: per effort setting, the defects
//! caught, the known false positives raised, the tokens and the dollars; and how
//! often `auto` chose a level below the lowest fixed level that caught every
//! defect of the case. A pure function over `ArmReport`s: it reads the matcher's
//! tallies and the usage totals, never a finding count or a token count alone.

use serde::Serialize;

use super::report::{Arm, ArmReport};
use super::setting::Setting;
use crate::budget::Effort;

/// One setting's totals over every case it ran, for one arm.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingTotals {
  /// The effort setting.
  pub setting: Setting,
  /// Baseline or enhanced.
  pub arm: Arm,
  /// Cases it ran.
  pub cases: usize,
  /// Defect expectations caught.
  pub defects_caught: usize,
  /// Defect expectations missed.
  pub defects_missed: usize,
  /// Known false positives raised.
  pub false_positives: usize,
  /// Prompt plus completion tokens, live or replayed.
  pub tokens: u64,
  /// Billed dollars; zero for replayed calls.
  pub cost_usd: f64,
}

/// What `auto` chose for one case, against the lowest fixed level that caught every
/// defect there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutoChoice {
  /// The case.
  pub case: String,
  /// Baseline or enhanced.
  pub arm: Arm,
  /// The level `auto` resolved to.
  pub chose: Effort,
  /// The lowest fixed level that caught every defect, when any did.
  pub lowest_catching: Option<Effort>,
  /// `auto` chose below `lowest_catching`.
  pub below: bool,
}

/// The calibration document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Calibration {
  /// Totals per setting and arm, in the order first seen.
  pub settings: Vec<SettingTotals>,
  /// One entry per case `auto` ran with a measurable fixed level beside it.
  pub auto: Vec<AutoChoice>,
  /// How many `auto` runs chose below the lowest catching level.
  pub auto_below_lowest: usize,
}

/// A level's place on the ladder; `auto` is not a level.
fn rank(level: Effort) -> u8 {
  match level {
    Effort::Auto => 0,
    Effort::Small => 1,
    Effort::Medium => 2,
    Effort::High => 3,
    Effort::Xhigh => 4,
  }
}

fn totals(cases: &[(&str, &[ArmReport])]) -> Vec<SettingTotals> {
  let mut all: Vec<SettingTotals> = Vec::new();
  for report in cases.iter().flat_map(|(_, arms)| arms.iter()) {
    let index = all
      .iter()
      .position(|row| row.setting == report.setting && row.arm == report.arm)
      .unwrap_or_else(|| {
        all.push(SettingTotals {
          setting: report.setting,
          arm: report.arm,
          cases: 0,
          defects_caught: 0,
          defects_missed: 0,
          false_positives: 0,
          tokens: 0,
          cost_usd: 0.0,
        });
        all.len() - 1
      });
    if let Some(row) = all.get_mut(index) {
      row.cases += 1;
      row.defects_caught += report.tally.caught;
      row.defects_missed += report.tally.missed;
      row.false_positives += report.tally.flagged;
      row.tokens += report
        .usage
        .tokens_in
        .saturating_add(report.usage.tokens_out);
      row.cost_usd += report.usage.cost_usd;
    }
  }
  all
}

/// The `auto` runs of one case, each against the fixed levels of the same arm.
fn auto_choices(case: &str, arms: &[ArmReport]) -> Vec<AutoChoice> {
  arms
    .iter()
    .filter(|report| report.setting == Setting::Auto)
    .filter_map(|auto| {
      let lowest = arms
        .iter()
        .filter(|report| {
          report.arm == auto.arm && report.setting.is_fixed_level() && report.tally.missed == 0
        })
        .map(|report| report.effort)
        .min_by_key(|level| rank(*level));
      let measurable = arms
        .iter()
        .any(|report| report.arm == auto.arm && report.setting.is_fixed_level());
      measurable.then(|| AutoChoice {
        case: case.to_owned(),
        arm: auto.arm,
        chose: auto.effort,
        lowest_catching: lowest,
        below: lowest.is_some_and(|lowest| rank(auto.effort) < rank(lowest)),
      })
    })
    .collect()
}

/// Calibrate over each case's arm reports.
pub fn calibrate(cases: &[(&str, &[ArmReport])]) -> Calibration {
  let auto: Vec<AutoChoice> = cases
    .iter()
    .flat_map(|(case, arms)| auto_choices(case, arms))
    .collect();
  Calibration {
    settings: totals(cases),
    auto_below_lowest: auto.iter().filter(|choice| choice.below).count(),
    auto,
  }
}

#[cfg(test)]
#[path = "tests/calibration_test.rs"]
mod tests;
