//! Which eval arms run and whether responses are replayed or live.

use toolu_runtime::env::Env;

use super::replay::Store;
use super::report::Arm;
use super::setting::Setting;

/// Which arms of the comparison to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmChoice {
  /// Baseline and enhanced.
  Both,
  /// The Jev judge off.
  Baseline,
  /// The Jev judge on.
  Enhanced,
}

impl ArmChoice {
  /// The name `--arm` uses.
  pub const NAMES: [&'static str; 3] = ["both", "baseline", "enhanced"];

  /// The choice named `name`.
  pub fn parse(name: &str) -> Option<ArmChoice> {
    match name {
      "both" => Some(ArmChoice::Both),
      "baseline" => Some(ArmChoice::Baseline),
      "enhanced" => Some(ArmChoice::Enhanced),
      _ => None,
    }
  }

  /// The arms to run at `setting`; the known-weak `none` runs the baseline only.
  pub(crate) fn arms(self, setting: Setting) -> Vec<Arm> {
    let wanted = match self {
      ArmChoice::Both => vec![Arm::Baseline, Arm::Enhanced],
      ArmChoice::Baseline => vec![Arm::Baseline],
      ArmChoice::Enhanced => vec![Arm::Enhanced],
    };
    wanted
      .into_iter()
      .filter(|arm| setting != Setting::None || *arm == Arm::Baseline)
      .collect()
  }
}

/// Where responses come from.
#[derive(Debug, Clone)]
pub enum Mode {
  /// Only the store; nothing is sent.
  Replay(Store),
  /// The real clients; each exchange is recorded when there is a store.
  Live(Option<Store>),
}

/// What one run does.
pub struct Plan<'a> {
  /// The process environment; a replay run never reads its key.
  pub env: &'a Env,
  /// Which arms.
  pub arms: ArmChoice,
  /// Which effort settings; one arm set per setting.
  pub settings: &'a [Setting],
  /// Replay or live.
  pub mode: &'a Mode,
  /// The review model's endpoint; tests aim it at a loopback origin.
  pub openrouter: toolu_openrouter::Config,
  /// Jev's endpoint; `None` keeps the System One default.
  pub jev: Option<toolu_jev_client::Config>,
}

#[cfg(test)]
#[path = "tests/arms_test.rs"]
mod tests;
