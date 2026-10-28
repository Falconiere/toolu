use std::path::PathBuf;

use super::run_case;
use crate::eval::{ArmChoice, Fixtures, Mode, Plan, Setting, Store};
use toolu_runtime::env::Env;

fn golden() -> Fixtures {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval");
  Fixtures::load(&root).unwrap()
}

#[test]
fn enhanced_at_the_known_weak_none_setting_runs_no_arm() {
  let fixtures = golden();
  let empty = tempfile::tempdir().unwrap();
  let mode = Mode::Replay(Store::new(empty.path()));
  let env = Env::from_pairs([("GIT_CONFIG_NOSYSTEM", "1")]);
  let plan = Plan {
    env: &env,
    arms: ArmChoice::Enhanced,
    settings: &[Setting::None],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  let case = fixtures
    .set()
    .cases
    .iter()
    .find(|case| case.id == "slice-underflow")
    .unwrap();
  let report = run_case(&fixtures, case, &plan).unwrap();
  assert_eq!(report.report.arms.len(), 0);
  assert_eq!(report.unverified.len(), 1);
  assert!(
    report.unverified[0].contains("no arm selected") && report.unverified[0].contains("Enhanced"),
    "{}",
    report.unverified[0]
  );
}
