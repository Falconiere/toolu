use toolu_github::RateLimit;

use super::low;
use crate::model::World;
use crate::model::{Action, Issue, Step, fresh};

#[test]
fn github_budget_holds_effects_only_below_a_live_floor() {
  let mut world = World::new(1_000);
  assert!(!low(&world));
  world.rest_rate = Some(RateLimit {
    remaining: Some(999),
    reset: Some(120),
    ..RateLimit::default()
  });
  assert!(low(&world));
  world.now_ms = 120_000;
  assert!(!low(&world));
  world.graphql_remaining = Some(499);
  world.graphql_reset_at = Some(180);
  assert!(low(&world));
  world.graphql_remaining = Some(500);
  assert!(!low(&world));
}

#[test]
fn github_budget_pauses_merges_but_allows_cleanup() {
  let mut world = World::new(1_000);
  world.draining = true;
  world.rest_rate = Some(RateLimit {
    remaining: Some(5),
    reset: Some(120),
    ..RateLimit::default()
  });
  let mut issue = Issue::blank("one", "epic", "/state", 0);
  issue.stage = "running".into();
  issue.pending = Some(fresh(Action::Merge, "token".into()));
  world.issues.insert("one".into(), issue);
  assert_eq!(crate::logic::next(&mut world), None);
  world.issues.get_mut("one").expect("issue").pending =
    Some(fresh(Action::Cleanup, "token".into()));
  assert!(
    matches!(crate::logic::next(&mut world), Some(Step::Journal(row)) if row.name == "cleanup-intent")
  );
}
