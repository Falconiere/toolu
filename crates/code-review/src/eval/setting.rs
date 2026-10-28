//! The effort settings an eval run covers: every production level, plus `none`, the
//! known-weak baseline calibration compares them against. `none` is not a production
//! level: it never reaches `--effort` of `run`, it only selects the lowest level's
//! budget with the Jev judge off (the OpenRouter client has no reasoning-off mode).

use serde::Serialize;

use crate::budget::Effort;

/// One `--effort` value of `eval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Setting {
  /// Jev picks the level per case.
  Auto,
  /// The `small` level.
  Small,
  /// The `medium` level.
  Medium,
  /// The `high` level.
  High,
  /// The `xhigh` level.
  Xhigh,
  /// The known-weak baseline: the lowest level's budget, never judged.
  None,
}

impl Setting {
  /// Every value `--effort` accepts of `eval`, in help order.
  pub const ALL: [Setting; 6] = [
    Setting::Auto,
    Setting::Small,
    Setting::Medium,
    Setting::High,
    Setting::Xhigh,
    Setting::None,
  ];

  /// The name `--effort` and the report use.
  pub fn name(self) -> &'static str {
    match self {
      Setting::Auto => "auto",
      Setting::Small => "small",
      Setting::Medium => "medium",
      Setting::High => "high",
      Setting::Xhigh => "xhigh",
      Setting::None => "none",
    }
  }

  /// The setting named `name`.
  pub fn parse(name: &str) -> Option<Setting> {
    Setting::ALL
      .into_iter()
      .find(|setting| setting.name() == name)
  }

  /// The production level this setting asks `review::run` for.
  pub fn flag(self) -> Effort {
    match self {
      Setting::Auto => Effort::Auto,
      Setting::Small | Setting::None => Effort::Small,
      Setting::Medium => Effort::Medium,
      Setting::High => Effort::High,
      Setting::Xhigh => Effort::Xhigh,
    }
  }

  /// Whether this is a fixed production level, one calibration measures `auto` against.
  pub fn is_fixed_level(self) -> bool {
    !matches!(self, Setting::Auto | Setting::None)
  }
}

#[cfg(test)]
#[path = "tests/setting_test.rs"]
mod tests;
