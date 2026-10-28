//! Replay eval cases where an arm fails closed as unverified.

use toolu_code_review::eval::{Arm, ArmChoice, Mode, Plan, Setting, Store, Verdict, run_all};
use toolu_http_test_support::{Fixture, Reply};

use super::golden;
use super::replay::{JEV, REVIEW, answer, keyless, record};

#[test]
fn a_failed_judge_call_makes_the_enhanced_arm_unverified_not_a_pass() {
  let fixtures = golden().expect("fixture");
  let origin = Fixture::start().expect("loopback");
  origin
    .route(REVIEW, Reply::new(200, answer()))
    .expect("route");
  origin
    .route(JEV, Reply::new(500, "down".to_owned()))
    .expect("route");
  let store = tempfile::tempdir().expect("fs");
  let live = record(&fixtures, &origin, store.path()).expect("live run");
  assert_eq!(live.report.arms.len(), 1, "{:?}", live.unverified);
  assert_eq!(
    live.report.arms.first().map(|arm| arm.arm),
    Some(Arm::Baseline)
  );
  let why = live.unverified.first().expect("enhanced is unverified");
  assert!(why.contains("Enhanced at small"), "{why}");
  assert!(why.contains("failed calls: judge call"), "{why}");
  assert_eq!(live.unverified.len(), 1, "{:?}", live.unverified);
}

#[test]
fn a_selection_that_yields_no_arm_is_unverified_never_satisfied() {
  let fixtures = golden().expect("fixture");
  let empty = tempfile::tempdir().expect("fs");
  let mode = Mode::Replay(Store::new(empty.path()));
  let plan = Plan {
    env: &keyless(),
    arms: ArmChoice::Enhanced,
    settings: &[Setting::None],
    mode: &mode,
    openrouter: toolu_openrouter::Config::default(),
    jev: None,
  };
  let suite = run_all(&fixtures, &plan).expect("shapes");
  assert!(suite.cases.iter().all(|case| case.report.arms.is_empty()));
  assert!(suite.cases.iter().all(|case| !case.unverified.is_empty()));
  assert_eq!(suite.verdict(), Verdict::Unverified);
}
