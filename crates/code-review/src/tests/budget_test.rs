use super::{Effort, Judge};

#[test]
fn every_level_round_trips_its_name() {
  for level in Effort::ALL {
    assert_eq!(Effort::parse(level.name()), Some(level));
  }
  assert_eq!(Effort::parse("huge"), None);
}

#[test]
fn auto_budgets_as_the_medium_fallback() {
  assert_eq!(Effort::FALLBACK, Effort::Medium);
  assert_eq!(Effort::Auto.budget(), Effort::Medium.budget());
}

#[test]
fn higher_levels_never_get_less() {
  let levels = [Effort::Small, Effort::Medium, Effort::High, Effort::Xhigh];
  for pair in levels.windows(2) {
    let (low, high) = (pair[0].budget(), pair[1].budget());
    assert!(low.source_file_bytes < high.source_file_bytes);
    assert!(low.source_package_bytes < high.source_package_bytes);
    assert!(low.max_tokens < high.max_tokens);
    assert!(low.deadline_secs < high.deadline_secs);
  }
}

#[test]
fn the_levels_match_the_epic_table() {
  let small = Effort::Small.budget();
  assert_eq!(
    (small.source_file_bytes, small.source_package_bytes),
    (8192, 16384)
  );
  assert_eq!(
    (small.judge, small.reasoning, small.max_tokens),
    (Judge::BlockingOnly, "low", 16_384)
  );
  let medium = Effort::Medium.budget();
  assert_eq!(
    (medium.judge, medium.reasoning, medium.deadline_secs),
    (Judge::AllWithRecheck, "medium", 360)
  );
  assert_eq!(Effort::High.budget().reasoning, "high");
  let xhigh = Effort::Xhigh.budget();
  assert_eq!(
    (
      xhigh.judge,
      xhigh.reasoning,
      xhigh.max_tokens,
      xhigh.deadline_secs
    ),
    (Judge::AllWithContext, "xhigh", 131_072, 1200)
  );
}
