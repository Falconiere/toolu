use super::Setting;
use crate::budget::Effort;

#[test]
fn every_name_round_trips_and_an_unknown_one_is_refused() {
  for setting in Setting::ALL {
    assert_eq!(Setting::parse(setting.name()), Some(setting));
  }
  assert_eq!(Setting::parse("huge"), None);
  let names: Vec<&str> = Setting::ALL.iter().map(|setting| setting.name()).collect();
  assert_eq!(names, ["auto", "small", "medium", "high", "xhigh", "none"]);
}

#[test]
fn none_borrows_the_lowest_budget_and_is_not_a_fixed_level() {
  assert_eq!(Setting::None.flag(), Effort::Small);
  assert_eq!(Setting::Auto.flag(), Effort::Auto);
  assert_eq!(Setting::Xhigh.flag(), Effort::Xhigh);
  let fixed: Vec<Setting> = Setting::ALL
    .into_iter()
    .filter(|setting| setting.is_fixed_level())
    .collect();
  assert_eq!(
    fixed,
    [
      Setting::Small,
      Setting::Medium,
      Setting::High,
      Setting::Xhigh
    ]
  );
  assert_eq!(
    serde_json::to_string(&Setting::None).expect("json"),
    "\"none\""
  );
}
