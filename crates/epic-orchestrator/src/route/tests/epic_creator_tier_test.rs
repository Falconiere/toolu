//! The epic-creator plugin's shipped example sub-issue routes as Standard.

use super::{Evidence, heuristic_tier};
use crate::epic_creator_rig::{example_title, fence};

#[test]
fn epic_creator_example_sub_issue_stays_under_the_routing_thresholds() {
  let body = fence("## Example sub-issue").unwrap();
  assert!(
    body.chars().count() < 1200,
    "{} chars",
    body.chars().count()
  );
  assert!(body.matches("- [ ]").count() < 6);
}

#[test]
fn epic_creator_example_sub_issue_routes_as_standard() {
  let evidence = Evidence {
    key: "o-r-40".into(),
    title: example_title().unwrap().into(),
    excerpt: fence("## Example sub-issue").unwrap().trim().into(),
    ..Evidence::default()
  };
  assert_eq!(heuristic_tier(&evidence), 1);
}
