use super::{Evidence, heuristic_tier, tier_from_score};

fn titled(title: &str) -> Evidence {
  Evidence {
    key: "a".into(),
    title: title.into(),
    ..Evidence::default()
  }
}

#[test]
fn a_jev_score_rounds_with_a_small_upward_bias() {
  assert_eq!(tier_from_score(0.0), 0);
  assert_eq!(tier_from_score(1.36), 2);
  assert_eq!(tier_from_score(1.3), 1);
  assert_eq!(tier_from_score(3.0), 3);
  assert_eq!(tier_from_score(-2.0), 0);
  assert_eq!(tier_from_score(9.0), 3);
}

#[test]
fn the_heuristic_reads_title_labels_body_and_unblocked_work() {
  assert_eq!(heuristic_tier(&titled("Fix typo in README")), 0);
  assert_eq!(heuristic_tier(&titled("Security hardening of tokens")), 3);
  assert_eq!(
    heuristic_tier(&Evidence {
      unblocks: 4,
      ..titled("Add flag")
    }),
    2
  );
  assert_eq!(heuristic_tier(&titled("Add flag")), 1);
  let tasks = "- [ ] a\n- [ ] b\n- [x] c\n- [ ] d\n- [ ] e\n- [ ] f\n";
  assert_eq!(
    heuristic_tier(&Evidence {
      excerpt: tasks.into(),
      ..titled("Add flag")
    }),
    2
  );
  assert_eq!(
    heuristic_tier(&Evidence {
      excerpt: "x".repeat(1201),
      ..titled("Add flag")
    }),
    2
  );
  assert_eq!(
    heuristic_tier(&Evidence {
      labels: vec!["chore".into()],
      ..titled("Add flag")
    }),
    0
  );
}
