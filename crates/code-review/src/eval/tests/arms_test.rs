use super::ArmChoice;
use crate::eval::{Arm, Setting};

#[test]
fn the_arm_names_parse_and_both_runs_each_arm_once() {
  assert_eq!(ArmChoice::NAMES, ["both", "baseline", "enhanced"]);
  for name in ArmChoice::NAMES {
    assert!(ArmChoice::parse(name).is_some(), "{name}");
  }
  assert_eq!(ArmChoice::parse("third"), None);
  assert_eq!(
    ArmChoice::Both.arms(Setting::Medium),
    [Arm::Baseline, Arm::Enhanced]
  );
  assert_eq!(ArmChoice::Baseline.arms(Setting::Auto), [Arm::Baseline]);
  assert_eq!(ArmChoice::Enhanced.arms(Setting::High), [Arm::Enhanced]);
}

#[test]
fn the_known_weak_none_setting_only_ever_runs_the_baseline() {
  assert_eq!(ArmChoice::Both.arms(Setting::None), [Arm::Baseline]);
  assert_eq!(ArmChoice::Baseline.arms(Setting::None), [Arm::Baseline]);
  assert_eq!(ArmChoice::Enhanced.arms(Setting::None), Vec::<Arm>::new());
}
